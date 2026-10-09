//! Static WGSL cost model over naga's validated IR. Counts texture-
//! sampled expressions per (entry point × loop trip bound) — the real
//! per-pixel cost driver — plus instruction counts.
//!
//! naga lowers `for`/`while` to `loop { if (!cond) { break; } … }`, so
//! trip bounds are extracted from the leading guard `If` whose accept
//! arm is (or ends in) `Break` over a counter updated by a constant
//! step in the continuing block. A loop with no reachable `Break` is
//! an explicit unbounded loop; a guard whose bound can't be proven
//! gets `fallback_loop_iterations`.

use naga::{Arena, BinaryOperator, Block, Expression, Function, Literal, Module, Statement};
use std::collections::HashMap;

/// Conservative trip bound when a loop's bound can't be statically
/// proven (data-dependent bounds, unproven guards, etc.).
const FALLBACK_LOOP_ITERATIONS: u32 = 64;
const TEX_SAMPLE_COST: u64 = 4;
const MAX_CALL_DEPTH: usize = 8;
/// Proving trip bounds looks back at most this many statements.
const LOOKBACK: usize = 64;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ShaderCost {
    /// Upper bound of texture-sampled ops per entry point invocation.
    pub texture_samples_per_pixel: u64,
    /// Flat expression-statement count (approximate instruction count).
    pub instructions: u64,
    /// A loop exists with no reachable `Break` — it cannot terminate.
    pub has_unbounded_loop: bool,
}

/// Estimate a module's worst entry point; None when module has no
/// entry points.
pub fn module_cost(m: &Module) -> ShaderCost {
    let mut worst = ShaderCost::default();
    for ep in &m.entry_points {
        let c = function_cost(m, &ep.function);
        if c.texture_samples_per_pixel > worst.texture_samples_per_pixel {
            worst.texture_samples_per_pixel = c.texture_samples_per_pixel;
        }
        worst.instructions = worst.instructions.max(c.instructions);
        worst.has_unbounded_loop |= c.has_unbounded_loop;
    }
    worst
}

fn function_cost(m: &Module, f: &Function) -> ShaderCost {
    let mut memo: HashMap<usize, ShaderCost> = HashMap::new();
    block_cost(m, f, &f.body, 1, 0, &mut memo, &[])
}

#[allow(clippy::too_many_arguments)]
fn block_cost(
    m: &Module,
    f: &Function,
    block: &Block,
    mult: u64,
    depth: usize,
    memo: &mut HashMap<usize, ShaderCost>,
    stores: &[(naga::Handle<naga::LocalVariable>, i64)],
) -> ShaderCost {
    let mut cost = ShaderCost::default();
    let mut stores = stores.to_vec();
    for (idx, stmt) in block.iter().enumerate() {
        match stmt {
            Statement::Emit(range) => {
                for h in range.clone() {
                    cost.instructions += mult;
                    cost.texture_samples_per_pixel += mult * expr_tex(&f.expressions, h);
                }
            }
            Statement::Block(b) => {
                cost.merge(&block_cost(m, f, b, mult, depth, memo, &stores));
            }
            Statement::If { accept, reject, .. } => {
                let a = block_cost(m, f, accept, mult, depth, memo, &stores);
                let r = block_cost(m, f, reject, mult, depth, memo, &stores);
                cost.merge_max(&a, &r);
            }
            Statement::Switch { cases, .. } => {
                let mut w = ShaderCost::default();
                for c in cases {
                    let cc = block_cost(m, f, &c.body, mult, depth, memo, &stores);
                    w.merge_max(&w.clone(), &cc);
                }
                cost.merge(&w);
            }
            Statement::Loop {
                body,
                continuing,
                break_if,
            } => {
                // naga lowers `for`/`while` to loop{ if (cond) {break}
                // or if (cont) {} else {break} }: find the guard.
                // (cond, negated): negated = break-when-true.
                let guard = break_if
                    .map(|h| (h, true))
                    .or_else(|| find_guard(f, body))
                    .or_else(|| find_guard(f, continuing));
                let can_exit = guard.is_some() || has_break(body) || has_break(continuing);
                if !can_exit {
                    cost.has_unbounded_loop = true;
                }
                let trip = u64::from(match guard {
                    Some((cond, negated)) => {
                        trip_bound(f, &block[..idx], continuing, cond, negated, &stores)
                    }
                    None => FALLBACK_LOOP_ITERATIONS,
                });
                cost.merge(&block_cost(m, f, body, mult * trip, depth, memo, &stores));
                cost.merge(&block_cost(
                    m,
                    f,
                    continuing,
                    mult * trip,
                    depth,
                    memo,
                    &stores,
                ));
            }
            Statement::Store { pointer, value } => {
                record_store(f, &mut stores, *pointer, *value);
                cost.instructions += mult;
            }
            Statement::Call { function, .. } => {
                let idx = function.index();
                if !memo.contains_key(&idx) && depth < MAX_CALL_DEPTH {
                    let callee = &m.functions[*function];
                    let c = fn_cost(m, callee, depth + 1, memo);
                    memo.insert(idx, c);
                }
                if let Some(c) = memo.get(&idx) {
                    cost.instructions += mult * c.instructions;
                    cost.texture_samples_per_pixel += mult * c.texture_samples_per_pixel;
                    cost.has_unbounded_loop |= c.has_unbounded_loop;
                } else {
                    // recursion/deep chain — pessimistic
                    cost.instructions += mult * 256;
                }
            }
            Statement::ImageStore { .. } => {
                cost.texture_samples_per_pixel += mult * TEX_SAMPLE_COST;
            }
            _ => {
                cost.instructions += mult;
            }
        }
    }
    cost
}

impl ShaderCost {
    fn merge(&mut self, o: &ShaderCost) {
        self.texture_samples_per_pixel += o.texture_samples_per_pixel;
        self.instructions += o.instructions;
        self.has_unbounded_loop |= o.has_unbounded_loop;
    }
    /// Branch: take the more expensive arm.
    fn merge_max(&mut self, a: &ShaderCost, b: &ShaderCost) {
        self.texture_samples_per_pixel +=
            a.texture_samples_per_pixel.max(b.texture_samples_per_pixel);
        self.instructions += a.instructions.max(b.instructions);
        self.has_unbounded_loop |= a.has_unbounded_loop || b.has_unbounded_loop;
    }
}

fn fn_cost(
    m: &Module,
    f: &Function,
    depth: usize,
    memo: &mut HashMap<usize, ShaderCost>,
) -> ShaderCost {
    block_cost(m, f, &f.body, 1, depth, memo, &[])
}

fn expr_tex(arena: &Arena<Expression>, h: naga::Handle<Expression>) -> u64 {
    match &arena[h] {
        Expression::ImageSample { .. } => TEX_SAMPLE_COST,
        Expression::ImageLoad { .. } => TEX_SAMPLE_COST,
        Expression::ImageQuery { .. } => TEX_SAMPLE_COST / 2,
        _ => 0,
    }
}

/// `true` when any `Break` can exit THIS loop — scans the body's
/// statements without descending into nested `Loop`s (their breaks
/// exit the nested loop only).
fn has_break(b: &Block) -> bool {
    for s in b {
        match s {
            Statement::Break => return true,
            Statement::Block(inner) => {
                if has_break(inner) {
                    return true;
                }
            }
            Statement::If { accept, reject, .. } => {
                if has_break(accept) || has_break(reject) {
                    return true;
                }
            }
            Statement::Switch { cases, .. } if cases.iter().any(|c| has_break(&c.body)) => {
                return true;
            }
            _ => {}
        }
    }
    false
}

/// The loop guard: an `If` at the top of the body where one arm is
/// exactly `Break` and the other is empty — naga's lowering of
/// `for`/`while` conditions. Returns (condition, break_when_true):
/// `if (cond) {break} else {}` breaks when true; `if (cond) {} else
/// {break}` breaks when false.
fn find_guard(f: &Function, b: &Block) -> Option<(naga::Handle<Expression>, bool)> {
    let _ = f;
    let arm_is_break = |blk: &Block| {
        blk.iter()
            .rev()
            .find(|x| !matches!(x, Statement::Emit(_)))
            .map(|x| matches!(x, Statement::Break))
            .unwrap_or(false)
            && blk
                .iter()
                .all(|x| matches!(x, Statement::Emit(_) | Statement::Break))
    };
    let arm_empty = |blk: &Block| blk.iter().all(|x| matches!(x, Statement::Emit(_)));
    for s in b {
        match s {
            // emit/spacing statements can precede the guard
            Statement::Emit(_) => continue,
            Statement::If {
                condition,
                accept,
                reject,
            } => {
                if arm_is_break(accept) && arm_empty(reject) {
                    return Some((*condition, true));
                }
                if arm_empty(accept) && arm_is_break(reject) {
                    return Some((*condition, false));
                }
                return None;
            }
            _ => return None,
        }
    }
    None
}

/// Track `Store` ops assigning const-foldable literals to locals so a
/// later loop bound can be resolved.
fn record_store(
    f: &Function,
    stores: &mut Vec<(naga::Handle<naga::LocalVariable>, i64)>,
    pointer: naga::Handle<Expression>,
    value: naga::Handle<Expression>,
) {
    let var = match f.expressions[pointer] {
        Expression::LocalVariable(v) => v,
        _ => return,
    };
    if let Some(v) = lit_i64(&f.expressions[value]) {
        stores.retain(|(s, _)| *s != var);
        stores.push((var, v));
        if stores.len() > 128 {
            stores.remove(0);
        }
    }
}

/// Trip bound from the guard condition — must be a const comparison
/// over a counter the continuing block updates with a constant
/// add/sub step, initialized by a const store earlier in the
/// enclosing block (the `for` init clause lands there).
fn trip_bound(
    f: &Function,
    pre: &[Statement],
    continuing: &Block,
    cond: naga::Handle<Expression>,
    negated: bool,
    init_stores: &[(naga::Handle<naga::LocalVariable>, i64)],
) -> u32 {
    let Expression::Binary { op, left, right } = f.expressions[cond] else {
        return FALLBACK_LOOP_ITERATIONS;
    };
    // `negated` (break-when-true) inverts the comparison so `op`
    // below always describes the *continue* condition.
    let op = if negated {
        match op {
            BinaryOperator::Less => BinaryOperator::GreaterEqual,
            BinaryOperator::LessEqual => BinaryOperator::Greater,
            BinaryOperator::Greater => BinaryOperator::LessEqual,
            BinaryOperator::GreaterEqual => BinaryOperator::Less,
            BinaryOperator::Equal => BinaryOperator::NotEqual,
            BinaryOperator::NotEqual => BinaryOperator::Equal,
            _ => return FALLBACK_LOOP_ITERATIONS,
        }
    } else {
        op
    };
    // Load(local) vs Literal in either order
    let (var, bound, flip) = match (&f.expressions[left], &f.expressions[right]) {
        (Expression::Load { pointer }, _) => {
            let Expression::LocalVariable(v) = f.expressions[*pointer] else {
                return FALLBACK_LOOP_ITERATIONS;
            };
            match lit_i64(&f.expressions[right]) {
                Some(c) => (v, c, false),
                None => return FALLBACK_LOOP_ITERATIONS,
            }
        }
        (_, Expression::Load { pointer }) => {
            let Expression::LocalVariable(v) = f.expressions[*pointer] else {
                return FALLBACK_LOOP_ITERATIONS;
            };
            match lit_i64(&f.expressions[left]) {
                Some(c) => (v, c, true),
                None => return FALLBACK_LOOP_ITERATIONS,
            }
        }
        _ => return FALLBACK_LOOP_ITERATIONS,
    };
    // seed: `var i = K` lands on LocalVariable.init in this IR, or a
    // const Store earlier in the enclosing block (`for` header init).
    let mut init = init_stores
        .iter()
        .rfind(|(s, _)| *s == var)
        .map(|(_, v)| *v);
    if init.is_none() {
        init = f.local_variables[var]
            .init
            .and_then(|h| lit_i64(&f.expressions[h]));
    }
    for stmt in pre.iter().rev().take(LOOKBACK) {
        if init.is_some() {
            break;
        }
        if let Statement::Store { pointer, value } = stmt {
            if let Expression::LocalVariable(v) = f.expressions[*pointer] {
                if v == var {
                    init = lit_i64(&f.expressions[*value]);
                }
            }
        }
    }
    // step: continuing block's `Store var = var +/- const`
    let mut step: Option<i64> = None;
    for stmt in continuing {
        if let Statement::Store { pointer, value } = stmt {
            if let Expression::LocalVariable(v) = f.expressions[*pointer] {
                if v == var {
                    if let Expression::Binary { op, left, right } = f.expressions[*value] {
                        let l_ok = matches!(
                            (&f.expressions[left], lit_i64(&f.expressions[right])),
                            (Expression::Load { pointer: p }, Some(_))
                                if matches!(f.expressions[*p], Expression::LocalVariable(x) if x == var)
                        );
                        if l_ok {
                            let d = lit_i64(&f.expressions[right]).unwrap();
                            step = match op {
                                BinaryOperator::Add => Some(d),
                                BinaryOperator::Subtract => Some(-d),
                                _ => None,
                            };
                        }
                    }
                }
            }
        }
    }
    let (Some(init), Some(step)) = (init, step) else {
        return FALLBACK_LOOP_ITERATIONS;
    };
    if step == 0 {
        return FALLBACK_LOOP_ITERATIONS;
    }
    let (mut lo, mut hi, st) = if step > 0 {
        (init, bound, step)
    } else {
        (bound, init, -step)
    };
    // normalize to "monotone count toward bound"
    let lt = matches!(
        (op, flip, step > 0),
        (BinaryOperator::Less, false, true)
            | (BinaryOperator::LessEqual, false, true)
            | (BinaryOperator::Greater, true, true)
            | (BinaryOperator::GreaterEqual, true, true)
            | (BinaryOperator::Greater, false, false)
            | (BinaryOperator::GreaterEqual, false, false)
            | (BinaryOperator::Less, true, false)
            | (BinaryOperator::LessEqual, true, false)
    );
    if !lt {
        return FALLBACK_LOOP_ITERATIONS;
    }
    if matches!(
        (op, flip),
        (BinaryOperator::LessEqual, false)
            | (BinaryOperator::GreaterEqual, false)
            | (BinaryOperator::GreaterEqual, true)
            | (BinaryOperator::LessEqual, true)
    ) {
        if st > 0 {
            hi += 1;
        } else {
            lo -= 1;
        }
    }
    let span = hi - lo;
    if span <= 0 {
        return 0;
    }
    let iters = (span as u64).div_ceil(st as u64);
    u32::try_from(iters).unwrap_or(u32::MAX)
}

fn lit_i64(e: &Expression) -> Option<i64> {
    match e {
        Expression::Literal(l) => match l {
            Literal::I32(v) => Some(*v as i64),
            Literal::U32(v) => Some(*v as i64),
            Literal::I64(v) => Some(*v),
            Literal::U64(v) => i64::try_from(*v).ok(),
            Literal::AbstractInt(v) => Some(*v),
            _ => None,
        },
        _ => None,
    }
}
