// service/src/audio_capture.cpp
//
// WASAPI loopback capture.
// Requires COM initialized by caller (CoInitializeEx).
//
#include "audio_capture.h"
#include "com_ptr.h"
#include "owb_log.h"

#define WIN32_LEAN_AND_MEAN
#define NOMINMAX
#include <windows.h>
#include <mmdeviceapi.h>
#include <audioclient.h>
#include <functiondiscoverykeys_devpkey.h>

#include <algorithm>
#include <cwchar>

namespace owb {

struct AudioCapture::Impl {
    ComPtr<IMMDeviceEnumerator> enumerator;
    ComPtr<IMMDevice>           device;
    ComPtr<IAudioClient>        client;
    ComPtr<IAudioCaptureClient> capture;
    WAVEFORMATEX*               mix_format = nullptr;
    int                         sample_rate_ = 44100;
    int                         channels_    = 2;
    bool                        running_     = false;
    bool                        get_buffer_logged_ = false;
};

AudioCapture::AudioCapture() : impl_(std::make_unique<Impl>()) {}
AudioCapture::~AudioCapture() { stop(); }

// Releases all COM objects and frees mix_format.
// Safe to call whether or not start() succeeded.
void AudioCapture::release_impl(Impl* impl) {
    if (impl->mix_format) {
        CoTaskMemFree(impl->mix_format);
        impl->mix_format = nullptr;
    }
    impl->capture.reset();
    impl->client.reset();
    impl->device.reset();
    impl->enumerator.reset();
    impl->running_ = false;
    impl->get_buffer_logged_ = false;
}

bool AudioCapture::start() {
    if (impl_->running_) return true;

    HRESULT hr = CoCreateInstance(
        __uuidof(MMDeviceEnumerator), nullptr, CLSCTX_ALL,
        __uuidof(IMMDeviceEnumerator),
        reinterpret_cast<void**>(&impl_->enumerator.p)
    );
    if (FAILED(hr)) {
        OWB_LOG_ERROR("CoCreateInstance(MMDeviceEnumerator) failed: 0x%08lx", (unsigned long)hr);
        release_impl(impl_.get());
        return false;
    }

    hr = impl_->enumerator->GetDefaultAudioEndpoint(eRender, eConsole, &impl_->device.p);
    if (FAILED(hr)) {
        OWB_LOG_ERROR("GetDefaultAudioEndpoint failed: 0x%08lx", (unsigned long)hr);
        release_impl(impl_.get());
        return false;
    }

    endpoint_name_.clear();
    endpoint_is_bluetooth_ = false;
    {
        IPropertyStore* store = nullptr;
        if (SUCCEEDED(impl_->device->OpenPropertyStore(STGM_READ, &store)) && store) {
            PROPVARIANT value;
            PropVariantInit(&value);
            if (SUCCEEDED(store->GetValue(PKEY_Device_FriendlyName, &value)) &&
                value.vt == VT_LPWSTR && value.pwszVal) {
                const size_t len = std::wcslen(value.pwszVal);
                if (len > 512u) {
                    PropVariantClear(&value);
                    store->Release();
                    OWB_LOG_ERROR("Audio endpoint name is unexpectedly long");
                    release_impl(impl_.get());
                    return false;
                }
                const int bytes = WideCharToMultiByte(
                    CP_UTF8, 0, value.pwszVal, static_cast<int>(len),
                    nullptr, 0, nullptr, nullptr);
                if (bytes > 0) {
                    endpoint_name_.resize(static_cast<size_t>(bytes));
                    (void)WideCharToMultiByte(
                        CP_UTF8, 0, value.pwszVal, static_cast<int>(len),
                        endpoint_name_.data(), bytes, nullptr, nullptr);
                }
            }
            PROPVARIANT enumerator_value;
            PropVariantInit(&enumerator_value);
            if (SUCCEEDED(store->GetValue(PKEY_Device_EnumeratorName, &enumerator_value)) &&
                enumerator_value.vt == VT_LPWSTR && enumerator_value.pwszVal &&
                (_wcsicmp(enumerator_value.pwszVal, L"BTHENUM") == 0 ||
                 _wcsicmp(enumerator_value.pwszVal, L"BTHLEENUM") == 0)) {
                endpoint_is_bluetooth_ = true;
            }
            PropVariantClear(&enumerator_value);
            PropVariantClear(&value);
            store->Release();
        }
    }
    if (!endpoint_name_.empty()) {
        OWB_LOG_INFO("Capture endpoint: %s (Bluetooth=%d)",
                     endpoint_name_.c_str(), endpoint_is_bluetooth_ ? 1 : 0);
    }

    hr = impl_->device->Activate(
        __uuidof(IAudioClient), CLSCTX_ALL, nullptr,
        reinterpret_cast<void**>(&impl_->client.p)
    );
    if (FAILED(hr)) {
        OWB_LOG_ERROR("Activate(IAudioClient) failed: 0x%08lx", (unsigned long)hr);
        release_impl(impl_.get());
        return false;
    }

    hr = impl_->client->GetMixFormat(&impl_->mix_format);
    if (FAILED(hr)) {
        OWB_LOG_ERROR("GetMixFormat failed: 0x%08lx", (unsigned long)hr);
        release_impl(impl_.get());
        return false;
    }

    impl_->sample_rate_ = static_cast<int>(impl_->mix_format->nSamplesPerSec);
    impl_->channels_    = static_cast<int>(impl_->mix_format->nChannels);
    // The loopback mix can be multichannel (for example 5.1). The pipeline
    // downmixes the first two channels to stereo. Require a complete 32-bit
    // IEEE-float frame so every packet can be converted without truncation.
    if (impl_->mix_format->wFormatTag != WAVE_FORMAT_IEEE_FLOAT ||
        impl_->mix_format->wBitsPerSample != 32 ||
        impl_->channels_ <= 0 ||
        impl_->mix_format->nBlockAlign != impl_->mix_format->nChannels * 4u) {
        OWB_LOG_ERROR("Unsupported WASAPI mix format tag=%u bits=%u channels=%u",
                      impl_->mix_format->wFormatTag,
                      impl_->mix_format->wBitsPerSample,
                      impl_->mix_format->nChannels);
        release_impl(impl_.get());
        return false;
    }

    // Capture in 20 ms periods. The old 100 ms period could judder the stream
    // and forced larger pipeline bursts. 20 ms still leaves ample scheduling
    // slack for loopback capture without sending large irregular packets.
    constexpr REFERENCE_TIME kBufDuration = 200000; // 20 ms
    hr = impl_->client->Initialize(
        AUDCLNT_SHAREMODE_SHARED,
        AUDCLNT_STREAMFLAGS_LOOPBACK,
        kBufDuration, 0,
        impl_->mix_format, nullptr
    );
    if (FAILED(hr)) {
        OWB_LOG_ERROR("Initialize failed: 0x%08lx", (unsigned long)hr);
        release_impl(impl_.get());
        return false;
    }

    hr = impl_->client->GetService(
        __uuidof(IAudioCaptureClient),
        reinterpret_cast<void**>(&impl_->capture.p));
    if (FAILED(hr)) {
        OWB_LOG_ERROR("GetService(IAudioCaptureClient) failed: 0x%08lx", (unsigned long)hr);
        release_impl(impl_.get());
        return false;
    }

    hr = impl_->client->Start();
    if (FAILED(hr)) {
        OWB_LOG_ERROR("IAudioClient::Start failed: 0x%08lx", (unsigned long)hr);
        release_impl(impl_.get());
        return false;
    }

    OWB_LOG_INFO("Audio capture started: %u Hz, %u ch, %u bits, format 0x%u",
                 impl_->mix_format->nSamplesPerSec,
                 impl_->mix_format->nChannels,
                 impl_->mix_format->wBitsPerSample,
                 impl_->mix_format->wFormatTag);

    impl_->running_ = true;
    return true;
}

void AudioCapture::stop() {
    if (!impl_->running_) return;
    OWB_LOG_INFO("Stopping audio capture");
    impl_->client->Stop();
    release_impl(impl_.get());
}

std::ptrdiff_t AudioCapture::read(std::span<int16_t> buffer) {
    if (!impl_->running_ || !impl_->capture.p) return -1;

    BYTE*  data         = nullptr;
    UINT32 frames_avail = 0;
    DWORD  flags        = 0;

    HRESULT hr = impl_->capture->GetBuffer(&data, &frames_avail, &flags, nullptr, nullptr);
    if (hr == AUDCLNT_S_BUFFER_EMPTY || frames_avail == 0) return 0;
    if (FAILED(hr)) {
        if (!impl_->get_buffer_logged_) {
            OWB_LOG_ERROR("GetBuffer failed: 0x%08lx", (unsigned long)hr);
            impl_->get_buffer_logged_ = true;
        }
        return -1;
    }
    impl_->get_buffer_logged_ = false;

    const auto   ch            = static_cast<size_t>(impl_->channels_);
    if (ch == 0) {
        impl_->capture->ReleaseBuffer(frames_avail);
        return -1;
    }
    // The caller's buffer is sized for the capture period. If WASAPI returns
    // more, skip this packet and let the next read resynchronise.
    if (frames_avail > buffer.size() / ch) {
        impl_->capture->ReleaseBuffer(frames_avail);
        return 0;
    }

    const size_t samples_avail = frames_avail * ch;
    if (flags & AUDCLNT_BUFFERFLAGS_SILENT) {
        std::fill(buffer.begin(),
                  buffer.begin() + static_cast<std::ptrdiff_t>(samples_avail),
                  int16_t{0});
    } else {
        // WASAPI shared mode returns IEEE float32 in the validated mix format.
        const float* src = reinterpret_cast<const float*>(data);
        for (size_t i = 0; i < samples_avail; ++i) {
            float s = src[i];
            s = s > 1.0f ? 1.0f : (s < -1.0f ? -1.0f : s);
            buffer[i] = static_cast<int16_t>(s * 32767.0f);
        }
    }

    impl_->capture->ReleaseBuffer(frames_avail);
    return static_cast<std::ptrdiff_t>(frames_avail);
}

int AudioCapture::sample_rate() const noexcept { return impl_->sample_rate_; }
int AudioCapture::channels()    const noexcept { return impl_->channels_; }

} // namespace owb
