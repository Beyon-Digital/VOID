// Back-compat alias — `Button` was the pre-Signal name; the canonical family
// is ActionButton (same props + secondary/size additions).
import React from 'react';
import { ActionButton, type ActionButtonProps } from './ActionButton';

export interface ButtonProps extends ActionButtonProps {}

export const Button: React.FC<ButtonProps> = (props) => <ActionButton {...props} />;
