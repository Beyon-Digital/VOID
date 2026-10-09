// VOID engine — Unix-domain-socket control channel.
// Framing: 4-byte little-endian length prefix + payload (matches
// crates/void-protocol/src/frame.rs encode_frame / FrameReader).
#pragma once

#include <cstdint>
#include <string>
#include <vector>

namespace voidengine
{

/// Maximum accepted control-frame payload (mirrors CONTROL_FRAME_MAX).
inline constexpr uint32_t kControlFrameMax = 1024 * 1024;

/// Blocking client for one UDS connection (control or telemetry).
/// One instance per socket; writes are serialized by the owning thread —
/// do not share a channel across threads without an external mutex.
class UdsChannel
{
public:
    UdsChannel() = default;
    ~UdsChannel();

    UdsChannel (const UdsChannel&) = delete;
    UdsChannel& operator= (const UdsChannel&) = delete;
    UdsChannel (UdsChannel&& o) noexcept : fd_ (o.fd_), sendNonBlocking_ (o.sendNonBlocking_), rxBuf_ (std::move (o.rxBuf_)) { o.fd_ = -1; }
    UdsChannel& operator= (UdsChannel&& o) noexcept
    {
        close();
        fd_ = o.fd_; o.fd_ = -1;
        sendNonBlocking_ = o.sendNonBlocking_;
        rxBuf_ = std::move (o.rxBuf_);
        return *this;
    }

    /// Connect to an existing listener path. Returns false on failure.
    bool connectTo (const std::string& path);

    /// True while the socket is open.
    bool isOpen() const noexcept { return fd_ >= 0; }
    int  fd() const noexcept { return fd_; }

    /// Length-prefix + write the whole payload. Returns false on error.
    bool sendFrame (const uint8_t* data, size_t len);

    /// Blocking read of the next complete frame payload into `out`.
    /// Returns false on peer close / error.
    bool recvFrame (std::vector<uint8_t>& out);

    /// Set non-blocking mode for sends (telemetry: drop on EAGAIN).
    void setSendNonBlocking (bool nb);

    void close();

private:
    int fd_ = -1;
    bool sendNonBlocking_ = false;
    std::vector<uint8_t> rxBuf_;
};

} // namespace voidengine
