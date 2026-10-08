#include "UdsChannel.h"

#include <cerrno>
#include <cstring>
#include <fcntl.h>
#include <sys/socket.h>
#include <sys/un.h>
#include <unistd.h>

namespace voidengine
{

UdsChannel::~UdsChannel()
{
    close();
}

bool UdsChannel::connectTo (const std::string& path)
{
    close();
    fd_ = ::socket (AF_UNIX, SOCK_STREAM, 0);
    if (fd_ < 0)
        return false;

    sockaddr_un addr {};
    addr.sun_family = AF_UNIX;
    if (path.size() >= sizeof (addr.sun_path))
    {
        close();
        return false;
    }
    std::strncpy (addr.sun_path, path.c_str(), sizeof (addr.sun_path) - 1);

    if (::connect (fd_, reinterpret_cast<sockaddr*> (&addr), sizeof (addr)) != 0)
    {
        close();
        return false;
    }
    return true;
}

void UdsChannel::setSendNonBlocking (bool nb)
{
    sendNonBlocking_ = nb;
    if (fd_ < 0)
        return;
    int flags = fcntl (fd_, F_GETFL, 0);
    if (nb)
        fcntl (fd_, F_SETFL, flags | O_NONBLOCK);
    else
        fcntl (fd_, F_SETFL, flags & ~O_NONBLOCK);
}

bool UdsChannel::sendFrame (const uint8_t* data, size_t len)
{
    if (fd_ < 0 || len > kControlFrameMax)
        return false;

    uint8_t header[4];
    const auto n = static_cast<uint32_t> (len);
    header[0] = static_cast<uint8_t> (n & 0xff);
    header[1] = static_cast<uint8_t> ((n >> 8) & 0xff);
    header[2] = static_cast<uint8_t> ((n >> 16) & 0xff);
    header[3] = static_cast<uint8_t> ((n >> 24) & 0xff);

    auto writeAll = [this] (const uint8_t* p, size_t remaining) -> bool
    {
        while (remaining > 0)
        {
            const ssize_t w = ::send (fd_, p, remaining, 0);
            if (w < 0)
            {
                if (errno == EINTR)
                    continue;
                if (sendNonBlocking_ && (errno == EAGAIN || errno == EWOULDBLOCK))
                    return false; // lossy channel: caller drops the frame
                return false;
            }
            p += w;
            remaining -= static_cast<size_t> (w);
        }
        return true;
    };

    if (! writeAll (header, 4))
        return false;
    return writeAll (data, len);
}

bool UdsChannel::recvFrame (std::vector<uint8_t>& out)
{
    if (fd_ < 0)
        return false;

    // 1) Try to decode a complete frame from buffered bytes.
    auto tryDecode = [&]() -> bool
    {
        if (rxBuf_.size() < 4)
            return false;
        const uint32_t declared = static_cast<uint32_t> (rxBuf_[0])
                                | (static_cast<uint32_t> (rxBuf_[1]) << 8)
                                | (static_cast<uint32_t> (rxBuf_[2]) << 16)
                                | (static_cast<uint32_t> (rxBuf_[3]) << 24);
        if (declared > kControlFrameMax)
            return false; // protocol violation — caller closes
        if (rxBuf_.size() < 4 + declared)
            return false;
        out.assign (rxBuf_.begin() + 4, rxBuf_.begin() + 4 + declared);
        rxBuf_.erase (rxBuf_.begin(), rxBuf_.begin() + 4 + declared);
        return true;
    };

    if (tryDecode())
        return true;

    uint8_t chunk[16384];
    for (;;)
    {
        const ssize_t r = ::recv (fd_, chunk, sizeof (chunk), 0);
        if (r < 0)
        {
            if (errno == EINTR)
                continue;
            return false;
        }
        if (r == 0)
            return false; // peer closed
        rxBuf_.insert (rxBuf_.end(), chunk, chunk + r);
        if (tryDecode())
            return true;
    }
}

void UdsChannel::close()
{
    if (fd_ >= 0)
    {
        ::close (fd_);
        fd_ = -1;
    }
    rxBuf_.clear();
}

} // namespace voidengine
