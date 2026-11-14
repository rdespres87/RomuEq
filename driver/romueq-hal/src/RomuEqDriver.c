//
// RomuEqDriver.c
// Native macOS CoreAudio HAL AudioServerPlugIn Virtual Loopback Driver for RomuEq.
//
// Copyright (c) 2026 RomuEq. All rights reserved.
// Clean-room implementation against Apple's <CoreAudio/AudioServerPlugIn.h> API.
//

#include <CoreAudio/AudioServerPlugIn.h>
#include <CoreFoundation/CoreFoundation.h>
#include <mach/mach_time.h>
#include <pthread.h>
#include <stdatomic.h>
#include <math.h>
#include <string.h>

// ============================================================================
// Constants & Object IDs
// ============================================================================

enum {
    kObjectID_PlugIn        = kAudioObjectPlugInObject, // 1
    kObjectID_Device        = 2,
    kObjectID_Stream_Input  = 3,
    kObjectID_Stream_Output = 4,
    kObjectID_Volume_Output = 5,
    kObjectID_Mute_Output   = 6,
};

#define kDeviceName         "Equaform-8"
#define kManufacturerName   "Equaform Audio"
#define kDeviceUID          "com.equaform.audio.device"
#define kModelUID           "com.equaform.audio.model"

#define kChannelCount       2U
#define kRingBufferFrames   65536U
#define kRingBufferMask     (kRingBufferFrames - 1U)

#define kMinVolumeDb        (-96.0f)
#define kMaxVolumeDb        (0.0f)

static const Float64 kSupportedSampleRates[] = {
    44100.0,
    48000.0,
    88200.0,
    96000.0,
    176400.0,
    192000.0,
};
static const UInt32 kSupportedSampleRatesCount =
    (UInt32)(sizeof(kSupportedSampleRates) / sizeof(kSupportedSampleRates[0]));

// ============================================================================
// Driver State
// ============================================================================

static AudioServerPlugInHostRef gPlugInHost = NULL;
static pthread_mutex_t gStateMutex = PTHREAD_MUTEX_INITIALIZER;

static _Atomic UInt32  gRefCount      = 0;
static _Atomic UInt32  gIOClientCount = 0;
static _Atomic Float64 gSampleRate    = 48000.0;

// Lock-free sequence lock for real-time clock state
static _Atomic UInt32  gClockSeq      = 0;
static UInt64          gAnchorHostTime = 0;
static Float64         gHostTicksPerFrame = 0.0;
static UInt64          gClockSeed     = 1;

static _Atomic Float32 gVolumeScalar  = 1.0f;
static _Atomic UInt32  gMuteValue     = 0;

// Interleaved stereo Float32 ring buffer + frame timestamp tag to avoid stale playback
static float           gRingBuffer[kRingBufferFrames * kChannelCount];
static _Atomic UInt64  gFrameStamp[kRingBufferFrames];

// ============================================================================
// Helper Functions
// ============================================================================

static void RomuEq_UpdateClock(Float64 sampleRate) {
    mach_timebase_info_data_t timebase;
    mach_timebase_info(&timebase);
    if (timebase.numer == 0) {
        timebase.numer = 1;
        timebase.denom = 1;
    }
    Float64 ticksPerSecond = (1000000000.0 * (Float64)timebase.denom) / (Float64)timebase.numer;
    Float64 ticksPerFrame  = ticksPerSecond / sampleRate;
    UInt64 now = mach_absolute_time();

    // Begin sequence lock write (odd sequence number)
    atomic_fetch_add_explicit(&gClockSeq, 1, memory_order_release);
    gHostTicksPerFrame = ticksPerFrame;
    gAnchorHostTime    = now;
    gClockSeed++;
    // End sequence lock write (even sequence number)
    atomic_fetch_add_explicit(&gClockSeq, 1, memory_order_release);
}

static Boolean RomuEq_IsValidSampleRate(Float64 sr) {
    for (UInt32 i = 0; i < kSupportedSampleRatesCount; ++i) {
        if (fabs(sr - kSupportedSampleRates[i]) < 0.5) {
            return true;
        }
    }
    return false;
}

static Float32 RomuEq_ScalarToDb(Float32 scalar) {
    if (isnan(scalar) || scalar <= 0.00001f) {
        return kMinVolumeDb;
    }
    if (scalar >= 1.0f) {
        return kMaxVolumeDb;
    }
    Float32 db = 20.0f * log10f(scalar);
    if (isnan(db) || db < kMinVolumeDb) db = kMinVolumeDb;
    if (db > kMaxVolumeDb) db = kMaxVolumeDb;
    return db;
}

static Float32 RomuEq_DbToScalar(Float32 db) {
    if (isnan(db) || db <= kMinVolumeDb) {
        return 0.0f;
    }
    if (db >= kMaxVolumeDb) {
        return 1.0f;
    }
    Float32 s = powf(10.0f, db / 20.0f);
    if (isnan(s) || s < 0.0f) return 0.0f;
    if (s > 1.0f) return 1.0f;
    return s;
}

static void RomuEq_FillStreamBasicDescription(AudioStreamBasicDescription* outDesc, Float64 sampleRate) {
    memset(outDesc, 0, sizeof(AudioStreamBasicDescription));
    outDesc->mSampleRate       = sampleRate;
    outDesc->mFormatID         = kAudioFormatLinearPCM;
    outDesc->mFormatFlags      = kAudioFormatFlagIsFloat | kAudioFormatFlagsNativeEndian | kAudioFormatFlagIsPacked;
    outDesc->mBytesPerPacket   = kChannelCount * sizeof(Float32);
    outDesc->mFramesPerPacket  = 1;
    outDesc->mBytesPerFrame    = kChannelCount * sizeof(Float32);
    outDesc->mChannelsPerFrame = kChannelCount;
    outDesc->mBitsPerChannel   = 32;
}

// ============================================================================
// Forward Declarations of AudioServerPlugInDriverInterface Methods
// ============================================================================

static HRESULT STDMETHODCALLTYPE RomuEq_QueryInterface(void* inDriver, REFIID inUUID, LPVOID* outInterface);
static ULONG   STDMETHODCALLTYPE RomuEq_AddRef(void* inDriver);
static ULONG   STDMETHODCALLTYPE RomuEq_Release(void* inDriver);
static OSStatus STDMETHODCALLTYPE RomuEq_Initialize(AudioServerPlugInDriverRef inDriver, AudioServerPlugInHostRef inHost);
static OSStatus STDMETHODCALLTYPE RomuEq_CreateDevice(AudioServerPlugInDriverRef inDriver, CFDictionaryRef inDescription, const AudioServerPlugInClientInfo* inClientInfo, AudioObjectID* outDeviceObjectID);
static OSStatus STDMETHODCALLTYPE RomuEq_DestroyDevice(AudioServerPlugInDriverRef inDriver, AudioObjectID inDeviceObjectID);
static OSStatus STDMETHODCALLTYPE RomuEq_AddDeviceClient(AudioServerPlugInDriverRef inDriver, AudioObjectID inDeviceObjectID, const AudioServerPlugInClientInfo* inClientInfo);
static OSStatus STDMETHODCALLTYPE RomuEq_RemoveDeviceClient(AudioServerPlugInDriverRef inDriver, AudioObjectID inDeviceObjectID, const AudioServerPlugInClientInfo* inClientInfo);
static OSStatus STDMETHODCALLTYPE RomuEq_PerformDeviceConfigurationChange(AudioServerPlugInDriverRef inDriver, AudioObjectID inDeviceObjectID, UInt64 inChangeAction, void* inChangeInfo);
static OSStatus STDMETHODCALLTYPE RomuEq_AbortDeviceConfigurationChange(AudioServerPlugInDriverRef inDriver, AudioObjectID inDeviceObjectID, UInt64 inChangeAction, void* inChangeInfo);
static Boolean  STDMETHODCALLTYPE RomuEq_HasProperty(AudioServerPlugInDriverRef inDriver, AudioObjectID inObjectID, pid_t inClientProcessID, const AudioObjectPropertyAddress* inAddress);
static OSStatus STDMETHODCALLTYPE RomuEq_IsPropertySettable(AudioServerPlugInDriverRef inDriver, AudioObjectID inObjectID, pid_t inClientProcessID, const AudioObjectPropertyAddress* inAddress, Boolean* outIsSettable);
static OSStatus STDMETHODCALLTYPE RomuEq_GetPropertyDataSize(AudioServerPlugInDriverRef inDriver, AudioObjectID inObjectID, pid_t inClientProcessID, const AudioObjectPropertyAddress* inAddress, UInt32 inQualifierDataSize, const void* inQualifierData, UInt32* outDataSize);
static OSStatus STDMETHODCALLTYPE RomuEq_GetPropertyData(AudioServerPlugInDriverRef inDriver, AudioObjectID inObjectID, pid_t inClientProcessID, const AudioObjectPropertyAddress* inAddress, UInt32 inQualifierDataSize, const void* inQualifierData, UInt32 inDataSize, UInt32* outDataSize, void* outData);
static OSStatus STDMETHODCALLTYPE RomuEq_SetPropertyData(AudioServerPlugInDriverRef inDriver, AudioObjectID inObjectID, pid_t inClientProcessID, const AudioObjectPropertyAddress* inAddress, UInt32 inQualifierDataSize, const void* inQualifierData, UInt32 inDataSize, const void* inData);
static OSStatus STDMETHODCALLTYPE RomuEq_StartIO(AudioServerPlugInDriverRef inDriver, AudioObjectID inDeviceObjectID, UInt32 inClientID);
static OSStatus STDMETHODCALLTYPE RomuEq_StopIO(AudioServerPlugInDriverRef inDriver, AudioObjectID inDeviceObjectID, UInt32 inClientID);
static OSStatus STDMETHODCALLTYPE RomuEq_GetZeroTimeStamp(AudioServerPlugInDriverRef inDriver, AudioObjectID inDeviceObjectID, UInt32 inClientID, Float64* outSampleTime, UInt64* outHostTime, UInt64* outSeed);
static OSStatus STDMETHODCALLTYPE RomuEq_WillDoIOOperation(AudioServerPlugInDriverRef inDriver, AudioObjectID inDeviceObjectID, UInt32 inClientID, UInt32 inOperationID, Boolean* outWillDo, Boolean* outWillDoInPlace);
static OSStatus STDMETHODCALLTYPE RomuEq_BeginIOOperation(AudioServerPlugInDriverRef inDriver, AudioObjectID inDeviceObjectID, UInt32 inClientID, UInt32 inOperationID, UInt32 inIOBufferFrameSize, const AudioServerPlugInIOCycleInfo* inIOCycleInfo);
static OSStatus STDMETHODCALLTYPE RomuEq_DoIOOperation(AudioServerPlugInDriverRef inDriver, AudioObjectID inDeviceObjectID, AudioObjectID inStreamObjectID, UInt32 inClientID, UInt32 inOperationID, UInt32 inIOBufferFrameSize, const AudioServerPlugInIOCycleInfo* inIOCycleInfo, void* ioMainBuffer, void* ioSecondaryBuffer);
static OSStatus STDMETHODCALLTYPE RomuEq_EndIOOperation(AudioServerPlugInDriverRef inDriver, AudioObjectID inDeviceObjectID, UInt32 inClientID, UInt32 inOperationID, UInt32 inIOBufferFrameSize, const AudioServerPlugInIOCycleInfo* inIOCycleInfo);

static AudioServerPlugInDriverInterface gDriverInterface = {
    NULL,
    RomuEq_QueryInterface,
    RomuEq_AddRef,
    RomuEq_Release,
    RomuEq_Initialize,
    RomuEq_CreateDevice,
    RomuEq_DestroyDevice,
    RomuEq_AddDeviceClient,
    RomuEq_RemoveDeviceClient,
    RomuEq_PerformDeviceConfigurationChange,
    RomuEq_AbortDeviceConfigurationChange,
    RomuEq_HasProperty,
    RomuEq_IsPropertySettable,
    RomuEq_GetPropertyDataSize,
    RomuEq_GetPropertyData,
    RomuEq_SetPropertyData,
    RomuEq_StartIO,
    RomuEq_StopIO,
    RomuEq_GetZeroTimeStamp,
    RomuEq_WillDoIOOperation,
    RomuEq_BeginIOOperation,
    RomuEq_DoIOOperation,
    RomuEq_EndIOOperation
};

static AudioServerPlugInDriverInterface* gDriverInterfacePtr = &gDriverInterface;
static AudioServerPlugInDriverRef gDriverRef = &gDriverInterfacePtr;

// ============================================================================
// Factory Entry Point
// ============================================================================

__attribute__((visibility("default")))
void* EquaformDriver_Create(CFAllocatorRef inAllocator, CFUUIDRef inRequestedTypeUUID) {
    (void)inAllocator;
    if (CFEqual(inRequestedTypeUUID, kAudioServerPlugInTypeUUID)) {
        RomuEq_AddRef(gDriverRef);
        return gDriverRef;
    }
    return NULL;
}

__attribute__((visibility("default")))
void* RomuEqDriver_Create(CFAllocatorRef inAllocator, CFUUIDRef inRequestedTypeUUID) {
    return EquaformDriver_Create(inAllocator, inRequestedTypeUUID);
}

// ============================================================================
// IUnknown Implementation
// ============================================================================

static HRESULT STDMETHODCALLTYPE RomuEq_QueryInterface(void* inDriver, REFIID inUUID, LPVOID* outInterface) {
    if (inDriver != gDriverRef) {
        return E_INVALIDARG;
    }
    if (outInterface == NULL) {
        return E_POINTER;
    }
    CFUUIDRef requestedUUID = CFUUIDCreateFromUUIDBytes(NULL, inUUID);
    if (requestedUUID == NULL) {
        return E_NOINTERFACE;
    }
    Boolean matched = CFEqual(requestedUUID, IUnknownUUID) ||
                      CFEqual(requestedUUID, kAudioServerPlugInDriverInterfaceUUID);
    CFRelease(requestedUUID);

    if (matched) {
        RomuEq_AddRef(gDriverRef);
        *outInterface = gDriverRef;
        return S_OK;
    }
    *outInterface = NULL;
    return E_NOINTERFACE;
}

static ULONG STDMETHODCALLTYPE RomuEq_AddRef(void* inDriver) {
    if (inDriver != gDriverRef) {
        return 0;
    }
    return atomic_fetch_add_explicit(&gRefCount, 1, memory_order_relaxed) + 1;
}

static ULONG STDMETHODCALLTYPE RomuEq_Release(void* inDriver) {
    if (inDriver != gDriverRef) {
        return 0;
    }
    UInt32 prev = atomic_fetch_sub_explicit(&gRefCount, 1, memory_order_relaxed);
    return (prev > 0) ? (prev - 1) : 0;
}

// ============================================================================
// Lifecycle & Configuration Operations
// ============================================================================

static OSStatus STDMETHODCALLTYPE RomuEq_Initialize(AudioServerPlugInDriverRef inDriver, AudioServerPlugInHostRef inHost) {
    if (inDriver != gDriverRef) {
        return kAudioHardwareBadObjectError;
    }
    pthread_mutex_lock(&gStateMutex);
    gPlugInHost = inHost;
    memset(gRingBuffer, 0, sizeof(gRingBuffer));
    for (UInt32 i = 0; i < kRingBufferFrames; ++i) {
        atomic_store_explicit(&gFrameStamp[i], 0, memory_order_relaxed);
    }
    Float64 sr = atomic_load_explicit(&gSampleRate, memory_order_relaxed);
    RomuEq_UpdateClock(sr);
    pthread_mutex_unlock(&gStateMutex);
    return kAudioHardwareNoError;
}

static OSStatus STDMETHODCALLTYPE RomuEq_CreateDevice(AudioServerPlugInDriverRef inDriver, CFDictionaryRef inDescription, const AudioServerPlugInClientInfo* inClientInfo, AudioObjectID* outDeviceObjectID) {
    (void)inDriver; (void)inDescription; (void)inClientInfo; (void)outDeviceObjectID;
    return kAudioHardwareUnsupportedOperationError;
}

static OSStatus STDMETHODCALLTYPE RomuEq_DestroyDevice(AudioServerPlugInDriverRef inDriver, AudioObjectID inDeviceObjectID) {
    (void)inDriver; (void)inDeviceObjectID;
    return kAudioHardwareUnsupportedOperationError;
}

static OSStatus STDMETHODCALLTYPE RomuEq_AddDeviceClient(AudioServerPlugInDriverRef inDriver, AudioObjectID inDeviceObjectID, const AudioServerPlugInClientInfo* inClientInfo) {
    (void)inClientInfo;
    if (inDriver != gDriverRef || inDeviceObjectID != kObjectID_Device) {
        return kAudioHardwareBadObjectError;
    }
    return kAudioHardwareNoError;
}

static OSStatus STDMETHODCALLTYPE RomuEq_RemoveDeviceClient(AudioServerPlugInDriverRef inDriver, AudioObjectID inDeviceObjectID, const AudioServerPlugInClientInfo* inClientInfo) {
    (void)inClientInfo;
    if (inDriver != gDriverRef || inDeviceObjectID != kObjectID_Device) {
        return kAudioHardwareBadObjectError;
    }
    return kAudioHardwareNoError;
}

static OSStatus STDMETHODCALLTYPE RomuEq_PerformDeviceConfigurationChange(AudioServerPlugInDriverRef inDriver, AudioObjectID inDeviceObjectID, UInt64 inChangeAction, void* inChangeInfo) {
    (void)inChangeInfo;
    if (inDriver != gDriverRef || inDeviceObjectID != kObjectID_Device) {
        return kAudioHardwareBadObjectError;
    }
    if (inChangeAction > 0) {
        Float64 newRate = (Float64)inChangeAction;
        if (RomuEq_IsValidSampleRate(newRate)) {
            pthread_mutex_lock(&gStateMutex);
            atomic_store_explicit(&gSampleRate, newRate, memory_order_release);
            RomuEq_UpdateClock(newRate);
            memset(gRingBuffer, 0, sizeof(gRingBuffer));
            for (UInt32 i = 0; i < kRingBufferFrames; ++i) {
                atomic_store_explicit(&gFrameStamp[i], 0, memory_order_relaxed);
            }
            AudioServerPlugInHostRef host = gPlugInHost;
            pthread_mutex_unlock(&gStateMutex);

            if (host != NULL) {
                AudioObjectPropertyAddress changedDeviceAddrs[] = {
                    { kAudioDevicePropertyNominalSampleRate, kAudioObjectPropertyScopeGlobal, kAudioObjectPropertyElementMain },
                };
                host->PropertiesChanged(host, kObjectID_Device, 1, changedDeviceAddrs);

                AudioObjectPropertyAddress changedStreamAddrs[] = {
                    { kAudioStreamPropertyVirtualFormat,  kAudioObjectPropertyScopeGlobal, kAudioObjectPropertyElementMain },
                    { kAudioStreamPropertyPhysicalFormat, kAudioObjectPropertyScopeGlobal, kAudioObjectPropertyElementMain },
                };
                host->PropertiesChanged(host, kObjectID_Stream_Input,  2, changedStreamAddrs);
                host->PropertiesChanged(host, kObjectID_Stream_Output, 2, changedStreamAddrs);
            }
        }
    }
    return kAudioHardwareNoError;
}

static OSStatus STDMETHODCALLTYPE RomuEq_AbortDeviceConfigurationChange(AudioServerPlugInDriverRef inDriver, AudioObjectID inDeviceObjectID, UInt64 inChangeAction, void* inChangeInfo) {
    (void)inDriver; (void)inDeviceObjectID; (void)inChangeAction; (void)inChangeInfo;
    return kAudioHardwareNoError;
}

// ============================================================================
// Property Inspection & Mutation
// ============================================================================

static Boolean STDMETHODCALLTYPE RomuEq_HasProperty(AudioServerPlugInDriverRef inDriver, AudioObjectID inObjectID, pid_t inClientProcessID, const AudioObjectPropertyAddress* inAddress) {
    (void)inClientProcessID;
    if (inDriver != gDriverRef || inAddress == NULL) {
        return false;
    }

    switch (inObjectID) {
        case kObjectID_PlugIn:
            switch (inAddress->mSelector) {
                case kAudioObjectPropertyBaseClass:
                case kAudioObjectPropertyClass:
                case kAudioObjectPropertyOwner:
                case kAudioObjectPropertyManufacturer:
                case kAudioObjectPropertyOwnedObjects:
                case kAudioPlugInPropertyDeviceList:
                case kAudioPlugInPropertyTranslateUIDToDevice:
                case kAudioPlugInPropertyResourceBundle:
                    return true;
                default:
                    return false;
            }

        case kObjectID_Device:
            switch (inAddress->mSelector) {
                case kAudioObjectPropertyBaseClass:
                case kAudioObjectPropertyClass:
                case kAudioObjectPropertyOwner:
                case kAudioObjectPropertyName:
                case kAudioObjectPropertyManufacturer:
                case kAudioObjectPropertyOwnedObjects:
                case kAudioDevicePropertyDeviceUID:
                case kAudioDevicePropertyModelUID:
                case kAudioDevicePropertyTransportType:
                case kAudioDevicePropertyRelatedDevices:
                case kAudioDevicePropertyClockDomain:
                case kAudioDevicePropertyDeviceIsAlive:
                case kAudioDevicePropertyDeviceIsRunning:
                case kAudioDevicePropertyDeviceCanBeDefaultDevice:
                case kAudioDevicePropertyDeviceCanBeDefaultSystemDevice:
                case kAudioDevicePropertyLatency:
                case kAudioDevicePropertyStreams:
                case kAudioObjectPropertyControlList:
                case kAudioDevicePropertySafetyOffset:
                case kAudioDevicePropertyNominalSampleRate:
                case kAudioDevicePropertyAvailableNominalSampleRates:
                case kAudioDevicePropertyIsHidden:
                case kAudioDevicePropertyZeroTimeStampPeriod:
                case kAudioDevicePropertyPreferredChannelsForStereo:
                case kAudioDevicePropertyPreferredChannelLayout:
                    return true;
                default:
                    return false;
            }

        case kObjectID_Stream_Input:
        case kObjectID_Stream_Output:
            switch (inAddress->mSelector) {
                case kAudioObjectPropertyBaseClass:
                case kAudioObjectPropertyClass:
                case kAudioObjectPropertyOwner:
                case kAudioObjectPropertyOwnedObjects:
                case kAudioStreamPropertyIsActive:
                case kAudioStreamPropertyDirection:
                case kAudioStreamPropertyTerminalType:
                case kAudioStreamPropertyStartingChannel:
                case kAudioStreamPropertyLatency:
                case kAudioStreamPropertyVirtualFormat:
                case kAudioStreamPropertyPhysicalFormat:
                case kAudioStreamPropertyAvailableVirtualFormats:
                case kAudioStreamPropertyAvailablePhysicalFormats:
                    return true;
                default:
                    return false;
            }

        case kObjectID_Volume_Output:
            switch (inAddress->mSelector) {
                case kAudioObjectPropertyBaseClass:
                case kAudioObjectPropertyClass:
                case kAudioObjectPropertyOwner:
                case kAudioObjectPropertyOwnedObjects:
                case kAudioControlPropertyScope:
                case kAudioControlPropertyElement:
                case kAudioLevelControlPropertyScalarValue:
                case kAudioLevelControlPropertyDecibelValue:
                case kAudioLevelControlPropertyDecibelRange:
                case kAudioLevelControlPropertyConvertScalarToDecibels:
                case kAudioLevelControlPropertyConvertDecibelsToScalar:
                    return true;
                default:
                    return false;
            }

        case kObjectID_Mute_Output:
            switch (inAddress->mSelector) {
                case kAudioObjectPropertyBaseClass:
                case kAudioObjectPropertyClass:
                case kAudioObjectPropertyOwner:
                case kAudioObjectPropertyOwnedObjects:
                case kAudioControlPropertyScope:
                case kAudioControlPropertyElement:
                case kAudioBooleanControlPropertyValue:
                    return true;
                default:
                    return false;
            }

        default:
            return false;
    }
}

static OSStatus STDMETHODCALLTYPE RomuEq_IsPropertySettable(AudioServerPlugInDriverRef inDriver, AudioObjectID inObjectID, pid_t inClientProcessID, const AudioObjectPropertyAddress* inAddress, Boolean* outIsSettable) {
    (void)inClientProcessID;
    if (inDriver != gDriverRef || inAddress == NULL || outIsSettable == NULL) {
        return kAudioHardwareBadObjectError;
    }
    if (!RomuEq_HasProperty(inDriver, inObjectID, inClientProcessID, inAddress)) {
        return kAudioHardwareUnknownPropertyError;
    }

    *outIsSettable = false;

    if (inObjectID == kObjectID_Device) {
        if (inAddress->mSelector == kAudioDevicePropertyNominalSampleRate) {
            *outIsSettable = true;
        }
    } else if (inObjectID == kObjectID_Stream_Input || inObjectID == kObjectID_Stream_Output) {
        if (inAddress->mSelector == kAudioStreamPropertyVirtualFormat ||
            inAddress->mSelector == kAudioStreamPropertyPhysicalFormat) {
            *outIsSettable = true;
        }
    } else if (inObjectID == kObjectID_Volume_Output) {
        if (inAddress->mSelector == kAudioLevelControlPropertyScalarValue ||
            inAddress->mSelector == kAudioLevelControlPropertyDecibelValue) {
            *outIsSettable = true;
        }
    } else if (inObjectID == kObjectID_Mute_Output) {
        if (inAddress->mSelector == kAudioBooleanControlPropertyValue) {
            *outIsSettable = true;
        }
    }

    return kAudioHardwareNoError;
}

static OSStatus STDMETHODCALLTYPE RomuEq_GetPropertyDataSize(AudioServerPlugInDriverRef inDriver, AudioObjectID inObjectID, pid_t inClientProcessID, const AudioObjectPropertyAddress* inAddress, UInt32 inQualifierDataSize, const void* inQualifierData, UInt32* outDataSize) {
    (void)inClientProcessID; (void)inQualifierDataSize; (void)inQualifierData;
    if (inDriver != gDriverRef || inAddress == NULL || outDataSize == NULL) {
        return kAudioHardwareBadObjectError;
    }

    *outDataSize = 0;

    switch (inObjectID) {
        case kObjectID_PlugIn:
            switch (inAddress->mSelector) {
                case kAudioObjectPropertyBaseClass:
                case kAudioObjectPropertyClass:
                case kAudioObjectPropertyOwner:
                    *outDataSize = sizeof(AudioObjectID);
                    return kAudioHardwareNoError;
                case kAudioObjectPropertyManufacturer:
                case kAudioPlugInPropertyResourceBundle:
                    *outDataSize = sizeof(CFStringRef);
                    return kAudioHardwareNoError;
                case kAudioObjectPropertyOwnedObjects:
                case kAudioPlugInPropertyDeviceList:
                case kAudioPlugInPropertyTranslateUIDToDevice:
                    *outDataSize = sizeof(AudioObjectID);
                    return kAudioHardwareNoError;
                default:
                    return kAudioHardwareUnknownPropertyError;
            }

        case kObjectID_Device:
            switch (inAddress->mSelector) {
                case kAudioObjectPropertyBaseClass:
                case kAudioObjectPropertyClass:
                case kAudioObjectPropertyOwner:
                    *outDataSize = sizeof(AudioObjectID);
                    return kAudioHardwareNoError;
                case kAudioObjectPropertyName:
                case kAudioObjectPropertyManufacturer:
                case kAudioDevicePropertyDeviceUID:
                case kAudioDevicePropertyModelUID:
                    *outDataSize = sizeof(CFStringRef);
                    return kAudioHardwareNoError;
                case kAudioObjectPropertyOwnedObjects:
                    if (inAddress->mScope == kAudioObjectPropertyScopeInput) {
                        *outDataSize = sizeof(AudioObjectID);
                    } else if (inAddress->mScope == kAudioObjectPropertyScopeOutput) {
                        *outDataSize = 3 * sizeof(AudioObjectID);
                    } else {
                        *outDataSize = 4 * sizeof(AudioObjectID);
                    }
                    return kAudioHardwareNoError;
                case kAudioDevicePropertyTransportType:
                case kAudioDevicePropertyClockDomain:
                case kAudioDevicePropertyDeviceIsAlive:
                case kAudioDevicePropertyDeviceIsRunning:
                case kAudioDevicePropertyDeviceCanBeDefaultDevice:
                case kAudioDevicePropertyDeviceCanBeDefaultSystemDevice:
                case kAudioDevicePropertyLatency:
                case kAudioDevicePropertySafetyOffset:
                case kAudioDevicePropertyIsHidden:
                case kAudioDevicePropertyZeroTimeStampPeriod:
                    *outDataSize = sizeof(UInt32);
                    return kAudioHardwareNoError;
                case kAudioDevicePropertyRelatedDevices:
                    *outDataSize = sizeof(AudioObjectID);
                    return kAudioHardwareNoError;
                case kAudioDevicePropertyStreams:
                    if (inAddress->mScope == kAudioObjectPropertyScopeInput ||
                        inAddress->mScope == kAudioObjectPropertyScopeOutput) {
                        *outDataSize = sizeof(AudioObjectID);
                    } else {
                        *outDataSize = 2 * sizeof(AudioObjectID);
                    }
                    return kAudioHardwareNoError;
                case kAudioObjectPropertyControlList:
                    *outDataSize = 2 * sizeof(AudioObjectID);
                    return kAudioHardwareNoError;
                case kAudioDevicePropertyNominalSampleRate:
                    *outDataSize = sizeof(Float64);
                    return kAudioHardwareNoError;
                case kAudioDevicePropertyAvailableNominalSampleRates:
                    *outDataSize = kSupportedSampleRatesCount * sizeof(AudioValueRange);
                    return kAudioHardwareNoError;
                case kAudioDevicePropertyPreferredChannelsForStereo:
                    *outDataSize = 2 * sizeof(UInt32);
                    return kAudioHardwareNoError;
                case kAudioDevicePropertyPreferredChannelLayout:
                    *outDataSize = offsetof(AudioChannelLayout, mChannelDescriptions) +
                                   kChannelCount * sizeof(AudioChannelDescription);
                    return kAudioHardwareNoError;
                default:
                    return kAudioHardwareUnknownPropertyError;
            }

        case kObjectID_Stream_Input:
        case kObjectID_Stream_Output:
            switch (inAddress->mSelector) {
                case kAudioObjectPropertyBaseClass:
                case kAudioObjectPropertyClass:
                case kAudioObjectPropertyOwner:
                    *outDataSize = sizeof(AudioObjectID);
                    return kAudioHardwareNoError;
                case kAudioObjectPropertyOwnedObjects:
                    *outDataSize = 0;
                    return kAudioHardwareNoError;
                case kAudioStreamPropertyIsActive:
                case kAudioStreamPropertyDirection:
                case kAudioStreamPropertyTerminalType:
                case kAudioStreamPropertyStartingChannel:
                case kAudioStreamPropertyLatency:
                    *outDataSize = sizeof(UInt32);
                    return kAudioHardwareNoError;
                case kAudioStreamPropertyVirtualFormat:
                case kAudioStreamPropertyPhysicalFormat:
                    *outDataSize = sizeof(AudioStreamBasicDescription);
                    return kAudioHardwareNoError;
                case kAudioStreamPropertyAvailableVirtualFormats:
                case kAudioStreamPropertyAvailablePhysicalFormats:
                    *outDataSize = kSupportedSampleRatesCount * sizeof(AudioStreamRangedDescription);
                    return kAudioHardwareNoError;
                default:
                    return kAudioHardwareUnknownPropertyError;
            }

        case kObjectID_Volume_Output:
            switch (inAddress->mSelector) {
                case kAudioObjectPropertyBaseClass:
                case kAudioObjectPropertyClass:
                case kAudioObjectPropertyOwner:
                    *outDataSize = sizeof(AudioObjectID);
                    return kAudioHardwareNoError;
                case kAudioObjectPropertyOwnedObjects:
                    *outDataSize = 0;
                    return kAudioHardwareNoError;
                case kAudioControlPropertyScope:
                case kAudioControlPropertyElement:
                    *outDataSize = sizeof(UInt32);
                    return kAudioHardwareNoError;
                case kAudioLevelControlPropertyScalarValue:
                case kAudioLevelControlPropertyDecibelValue:
                case kAudioLevelControlPropertyConvertScalarToDecibels:
                case kAudioLevelControlPropertyConvertDecibelsToScalar:
                    *outDataSize = sizeof(Float32);
                    return kAudioHardwareNoError;
                case kAudioLevelControlPropertyDecibelRange:
                    *outDataSize = sizeof(AudioValueRange);
                    return kAudioHardwareNoError;
                default:
                    return kAudioHardwareUnknownPropertyError;
            }

        case kObjectID_Mute_Output:
            switch (inAddress->mSelector) {
                case kAudioObjectPropertyBaseClass:
                case kAudioObjectPropertyClass:
                case kAudioObjectPropertyOwner:
                    *outDataSize = sizeof(AudioObjectID);
                    return kAudioHardwareNoError;
                case kAudioObjectPropertyOwnedObjects:
                    *outDataSize = 0;
                    return kAudioHardwareNoError;
                case kAudioControlPropertyScope:
                case kAudioControlPropertyElement:
                case kAudioBooleanControlPropertyValue:
                    *outDataSize = sizeof(UInt32);
                    return kAudioHardwareNoError;
                default:
                    return kAudioHardwareUnknownPropertyError;
            }

        default:
            return kAudioHardwareBadObjectError;
    }
}

static OSStatus STDMETHODCALLTYPE RomuEq_GetPropertyData(AudioServerPlugInDriverRef inDriver, AudioObjectID inObjectID, pid_t inClientProcessID, const AudioObjectPropertyAddress* inAddress, UInt32 inQualifierDataSize, const void* inQualifierData, UInt32 inDataSize, UInt32* outDataSize, void* outData) {
    (void)inClientProcessID;
    if (inDriver != gDriverRef || inAddress == NULL || outDataSize == NULL || outData == NULL) {
        return kAudioHardwareBadObjectError;
    }

    switch (inObjectID) {
        // --------------------------------------------------------------------
        // Plug-In Object (ID 1)
        // --------------------------------------------------------------------
        case kObjectID_PlugIn:
            switch (inAddress->mSelector) {
                case kAudioObjectPropertyBaseClass:
                    if (inDataSize < sizeof(AudioClassID)) return kAudioHardwareBadPropertySizeError;
                    *((AudioClassID*)outData) = kAudioObjectClassID;
                    *outDataSize = sizeof(AudioClassID);
                    return kAudioHardwareNoError;

                case kAudioObjectPropertyClass:
                    if (inDataSize < sizeof(AudioClassID)) return kAudioHardwareBadPropertySizeError;
                    *((AudioClassID*)outData) = kAudioPlugInClassID;
                    *outDataSize = sizeof(AudioClassID);
                    return kAudioHardwareNoError;

                case kAudioObjectPropertyOwner:
                    if (inDataSize < sizeof(AudioObjectID)) return kAudioHardwareBadPropertySizeError;
                    *((AudioObjectID*)outData) = kAudioObjectUnknown;
                    *outDataSize = sizeof(AudioObjectID);
                    return kAudioHardwareNoError;

                case kAudioObjectPropertyManufacturer:
                    if (inDataSize < sizeof(CFStringRef)) return kAudioHardwareBadPropertySizeError;
                    *((CFStringRef*)outData) = CFStringCreateCopy(kCFAllocatorDefault, CFSTR(kManufacturerName));
                    *outDataSize = sizeof(CFStringRef);
                    return kAudioHardwareNoError;

                case kAudioObjectPropertyOwnedObjects:
                case kAudioPlugInPropertyDeviceList:
                    if (inDataSize < sizeof(AudioObjectID)) {
                        *outDataSize = 0;
                        return kAudioHardwareNoError;
                    }
                    *((AudioObjectID*)outData) = kObjectID_Device;
                    *outDataSize = sizeof(AudioObjectID);
                    return kAudioHardwareNoError;

                case kAudioPlugInPropertyTranslateUIDToDevice: {
                    if (inDataSize < sizeof(AudioObjectID)) return kAudioHardwareBadPropertySizeError;
                    if (inQualifierData == NULL || inQualifierDataSize < sizeof(CFStringRef)) {
                        return kAudioHardwareBadPropertySizeError;
                    }
                    CFStringRef uid = *((const CFStringRef*)inQualifierData);
                    if (uid != NULL && CFEqual(uid, CFSTR(kDeviceUID))) {
                        *((AudioObjectID*)outData) = kObjectID_Device;
                    } else {
                        *((AudioObjectID*)outData) = kAudioObjectUnknown;
                    }
                    *outDataSize = sizeof(AudioObjectID);
                    return kAudioHardwareNoError;
                }

                case kAudioPlugInPropertyResourceBundle:
                    if (inDataSize < sizeof(CFStringRef)) return kAudioHardwareBadPropertySizeError;
                    *((CFStringRef*)outData) = CFStringCreateCopy(kCFAllocatorDefault, CFSTR(""));
                    *outDataSize = sizeof(CFStringRef);
                    return kAudioHardwareNoError;

                default:
                    return kAudioHardwareUnknownPropertyError;
            }

        // --------------------------------------------------------------------
        // Device Object (ID 2 - "RomuEq")
        // --------------------------------------------------------------------
        case kObjectID_Device:
            switch (inAddress->mSelector) {
                case kAudioObjectPropertyBaseClass:
                    if (inDataSize < sizeof(AudioClassID)) return kAudioHardwareBadPropertySizeError;
                    *((AudioClassID*)outData) = kAudioObjectClassID;
                    *outDataSize = sizeof(AudioClassID);
                    return kAudioHardwareNoError;

                case kAudioObjectPropertyClass:
                    if (inDataSize < sizeof(AudioClassID)) return kAudioHardwareBadPropertySizeError;
                    *((AudioClassID*)outData) = kAudioDeviceClassID;
                    *outDataSize = sizeof(AudioClassID);
                    return kAudioHardwareNoError;

                case kAudioObjectPropertyOwner:
                    if (inDataSize < sizeof(AudioObjectID)) return kAudioHardwareBadPropertySizeError;
                    *((AudioObjectID*)outData) = kObjectID_PlugIn;
                    *outDataSize = sizeof(AudioObjectID);
                    return kAudioHardwareNoError;

                case kAudioObjectPropertyName:
                    if (inDataSize < sizeof(CFStringRef)) return kAudioHardwareBadPropertySizeError;
                    *((CFStringRef*)outData) = CFStringCreateCopy(kCFAllocatorDefault, CFSTR(kDeviceName));
                    *outDataSize = sizeof(CFStringRef);
                    return kAudioHardwareNoError;

                case kAudioObjectPropertyManufacturer:
                    if (inDataSize < sizeof(CFStringRef)) return kAudioHardwareBadPropertySizeError;
                    *((CFStringRef*)outData) = CFStringCreateCopy(kCFAllocatorDefault, CFSTR(kManufacturerName));
                    *outDataSize = sizeof(CFStringRef);
                    return kAudioHardwareNoError;

                case kAudioDevicePropertyDeviceUID:
                    if (inDataSize < sizeof(CFStringRef)) return kAudioHardwareBadPropertySizeError;
                    *((CFStringRef*)outData) = CFStringCreateCopy(kCFAllocatorDefault, CFSTR(kDeviceUID));
                    *outDataSize = sizeof(CFStringRef);
                    return kAudioHardwareNoError;

                case kAudioDevicePropertyModelUID:
                    if (inDataSize < sizeof(CFStringRef)) return kAudioHardwareBadPropertySizeError;
                    *((CFStringRef*)outData) = CFStringCreateCopy(kCFAllocatorDefault, CFSTR(kModelUID));
                    *outDataSize = sizeof(CFStringRef);
                    return kAudioHardwareNoError;

                case kAudioDevicePropertyTransportType:
                    if (inDataSize < sizeof(UInt32)) return kAudioHardwareBadPropertySizeError;
                    *((UInt32*)outData) = kAudioDeviceTransportTypeVirtual;
                    *outDataSize = sizeof(UInt32);
                    return kAudioHardwareNoError;

                case kAudioDevicePropertyRelatedDevices:
                    if (inDataSize < sizeof(AudioObjectID)) return kAudioHardwareBadPropertySizeError;
                    *((AudioObjectID*)outData) = kObjectID_Device;
                    *outDataSize = sizeof(AudioObjectID);
                    return kAudioHardwareNoError;

                case kAudioDevicePropertyClockDomain:
                case kAudioDevicePropertyLatency:
                case kAudioDevicePropertyIsHidden:
                    if (inDataSize < sizeof(UInt32)) return kAudioHardwareBadPropertySizeError;
                    *((UInt32*)outData) = 0;
                    *outDataSize = sizeof(UInt32);
                    return kAudioHardwareNoError;

                case kAudioDevicePropertySafetyOffset:
                    if (inDataSize < sizeof(UInt32)) return kAudioHardwareBadPropertySizeError;
                    *((UInt32*)outData) = 0;
                    *outDataSize = sizeof(UInt32);
                    return kAudioHardwareNoError;

                case kAudioDevicePropertyDeviceIsAlive:
                case kAudioDevicePropertyDeviceCanBeDefaultDevice:
                case kAudioDevicePropertyDeviceCanBeDefaultSystemDevice:
                    if (inDataSize < sizeof(UInt32)) return kAudioHardwareBadPropertySizeError;
                    *((UInt32*)outData) = 1;
                    *outDataSize = sizeof(UInt32);
                    return kAudioHardwareNoError;

                case kAudioDevicePropertyDeviceIsRunning:
                    if (inDataSize < sizeof(UInt32)) return kAudioHardwareBadPropertySizeError;
                    *((UInt32*)outData) = (atomic_load_explicit(&gIOClientCount, memory_order_acquire) > 0) ? 1 : 0;
                    *outDataSize = sizeof(UInt32);
                    return kAudioHardwareNoError;

                case kAudioDevicePropertyZeroTimeStampPeriod:
                    if (inDataSize < sizeof(UInt32)) return kAudioHardwareBadPropertySizeError;
                    *((UInt32*)outData) = kRingBufferFrames;
                    *outDataSize = sizeof(UInt32);
                    return kAudioHardwareNoError;

                case kAudioObjectPropertyOwnedObjects: {
                    const AudioObjectID inputObjs[]  = { kObjectID_Stream_Input };
                    const AudioObjectID outputObjs[] = { kObjectID_Stream_Output, kObjectID_Volume_Output, kObjectID_Mute_Output };
                    const AudioObjectID allObjs[]    = { kObjectID_Stream_Input, kObjectID_Stream_Output, kObjectID_Volume_Output, kObjectID_Mute_Output };
                    const AudioObjectID* src = allObjs;
                    UInt32 count = 4;
                    if (inAddress->mScope == kAudioObjectPropertyScopeInput) {
                        src = inputObjs;
                        count = 1;
                    } else if (inAddress->mScope == kAudioObjectPropertyScopeOutput) {
                        src = outputObjs;
                        count = 3;
                    }
                    UInt32 maxItems = inDataSize / sizeof(AudioObjectID);
                    if (count > maxItems) count = maxItems;
                    memcpy(outData, src, count * sizeof(AudioObjectID));
                    *outDataSize = count * sizeof(AudioObjectID);
                    return kAudioHardwareNoError;
                }

                case kAudioDevicePropertyStreams: {
                    AudioObjectID* dst = (AudioObjectID*)outData;
                    if (inAddress->mScope == kAudioObjectPropertyScopeInput) {
                        if (inDataSize < sizeof(AudioObjectID)) return kAudioHardwareBadPropertySizeError;
                        dst[0] = kObjectID_Stream_Input;
                        *outDataSize = sizeof(AudioObjectID);
                    } else if (inAddress->mScope == kAudioObjectPropertyScopeOutput) {
                        if (inDataSize < sizeof(AudioObjectID)) return kAudioHardwareBadPropertySizeError;
                        dst[0] = kObjectID_Stream_Output;
                        *outDataSize = sizeof(AudioObjectID);
                    } else {
                        UInt32 maxItems = inDataSize / sizeof(AudioObjectID);
                        if (maxItems >= 2) {
                            dst[0] = kObjectID_Stream_Input;
                            dst[1] = kObjectID_Stream_Output;
                            *outDataSize = 2 * sizeof(AudioObjectID);
                        } else if (maxItems == 1) {
                            dst[0] = kObjectID_Stream_Input;
                            *outDataSize = sizeof(AudioObjectID);
                        } else {
                            *outDataSize = 0;
                        }
                    }
                    return kAudioHardwareNoError;
                }

                case kAudioObjectPropertyControlList: {
                    const AudioObjectID ctrls[] = { kObjectID_Volume_Output, kObjectID_Mute_Output };
                    UInt32 count = inDataSize / sizeof(AudioObjectID);
                    if (count > 2) count = 2;
                    memcpy(outData, ctrls, count * sizeof(AudioObjectID));
                    *outDataSize = count * sizeof(AudioObjectID);
                    return kAudioHardwareNoError;
                }

                case kAudioDevicePropertyNominalSampleRate:
                    if (inDataSize < sizeof(Float64)) return kAudioHardwareBadPropertySizeError;
                    *((Float64*)outData) = atomic_load_explicit(&gSampleRate, memory_order_acquire);
                    *outDataSize = sizeof(Float64);
                    return kAudioHardwareNoError;

                case kAudioDevicePropertyAvailableNominalSampleRates: {
                    UInt32 count = inDataSize / sizeof(AudioValueRange);
                    if (count > kSupportedSampleRatesCount) count = kSupportedSampleRatesCount;
                    AudioValueRange* ranges = (AudioValueRange*)outData;
                    for (UInt32 i = 0; i < count; ++i) {
                        ranges[i].mMinimum = kSupportedSampleRates[i];
                        ranges[i].mMaximum = kSupportedSampleRates[i];
                    }
                    *outDataSize = count * sizeof(AudioValueRange);
                    return kAudioHardwareNoError;
                }

                case kAudioDevicePropertyPreferredChannelsForStereo:
                    if (inDataSize < 2 * sizeof(UInt32)) return kAudioHardwareBadPropertySizeError;
                    ((UInt32*)outData)[0] = 1;
                    ((UInt32*)outData)[1] = 2;
                    *outDataSize = 2 * sizeof(UInt32);
                    return kAudioHardwareNoError;

                case kAudioDevicePropertyPreferredChannelLayout: {
                    UInt32 needed = (UInt32)(offsetof(AudioChannelLayout, mChannelDescriptions) +
                                    kChannelCount * sizeof(AudioChannelDescription));
                    if (inDataSize < needed) return kAudioHardwareBadPropertySizeError;
                    AudioChannelLayout* layout = (AudioChannelLayout*)outData;
                    memset(layout, 0, needed);
                    layout->mChannelLayoutTag = kAudioChannelLayoutTag_UseChannelDescriptions;
                    layout->mNumberChannelDescriptions = kChannelCount;
                    layout->mChannelDescriptions[0].mChannelLabel = kAudioChannelLabel_Left;
                    layout->mChannelDescriptions[1].mChannelLabel = kAudioChannelLabel_Right;
                    *outDataSize = needed;
                    return kAudioHardwareNoError;
                }

                default:
                    return kAudioHardwareUnknownPropertyError;
            }

        // --------------------------------------------------------------------
        // Stream Objects (ID 3 Input / ID 4 Output)
        // --------------------------------------------------------------------
        case kObjectID_Stream_Input:
        case kObjectID_Stream_Output:
            switch (inAddress->mSelector) {
                case kAudioObjectPropertyBaseClass:
                    if (inDataSize < sizeof(AudioClassID)) return kAudioHardwareBadPropertySizeError;
                    *((AudioClassID*)outData) = kAudioObjectClassID;
                    *outDataSize = sizeof(AudioClassID);
                    return kAudioHardwareNoError;

                case kAudioObjectPropertyClass:
                    if (inDataSize < sizeof(AudioClassID)) return kAudioHardwareBadPropertySizeError;
                    *((AudioClassID*)outData) = kAudioStreamClassID;
                    *outDataSize = sizeof(AudioClassID);
                    return kAudioHardwareNoError;

                case kAudioObjectPropertyOwner:
                    if (inDataSize < sizeof(AudioObjectID)) return kAudioHardwareBadPropertySizeError;
                    *((AudioObjectID*)outData) = kObjectID_Device;
                    *outDataSize = sizeof(AudioObjectID);
                    return kAudioHardwareNoError;

                case kAudioObjectPropertyOwnedObjects:
                    *outDataSize = 0;
                    return kAudioHardwareNoError;

                case kAudioStreamPropertyIsActive:
                    if (inDataSize < sizeof(UInt32)) return kAudioHardwareBadPropertySizeError;
                    *((UInt32*)outData) = 1;
                    *outDataSize = sizeof(UInt32);
                    return kAudioHardwareNoError;

                case kAudioStreamPropertyDirection:
                    if (inDataSize < sizeof(UInt32)) return kAudioHardwareBadPropertySizeError;
                    *((UInt32*)outData) = (inObjectID == kObjectID_Stream_Input) ? 1 : 0;
                    *outDataSize = sizeof(UInt32);
                    return kAudioHardwareNoError;

                case kAudioStreamPropertyTerminalType:
                    if (inDataSize < sizeof(UInt32)) return kAudioHardwareBadPropertySizeError;
                    *((UInt32*)outData) = (inObjectID == kObjectID_Stream_Input)
                        ? kAudioStreamTerminalTypeMicrophone
                        : kAudioStreamTerminalTypeSpeaker;
                    *outDataSize = sizeof(UInt32);
                    return kAudioHardwareNoError;

                case kAudioStreamPropertyStartingChannel:
                    if (inDataSize < sizeof(UInt32)) return kAudioHardwareBadPropertySizeError;
                    *((UInt32*)outData) = 1;
                    *outDataSize = sizeof(UInt32);
                    return kAudioHardwareNoError;

                case kAudioStreamPropertyLatency:
                    if (inDataSize < sizeof(UInt32)) return kAudioHardwareBadPropertySizeError;
                    *((UInt32*)outData) = 0;
                    *outDataSize = sizeof(UInt32);
                    return kAudioHardwareNoError;

                case kAudioStreamPropertyVirtualFormat:
                case kAudioStreamPropertyPhysicalFormat: {
                    if (inDataSize < sizeof(AudioStreamBasicDescription)) return kAudioHardwareBadPropertySizeError;
                    Float64 sr = atomic_load_explicit(&gSampleRate, memory_order_acquire);
                    RomuEq_FillStreamBasicDescription((AudioStreamBasicDescription*)outData, sr);
                    *outDataSize = sizeof(AudioStreamBasicDescription);
                    return kAudioHardwareNoError;
                }

                case kAudioStreamPropertyAvailableVirtualFormats:
                case kAudioStreamPropertyAvailablePhysicalFormats: {
                    UInt32 count = inDataSize / sizeof(AudioStreamRangedDescription);
                    if (count > kSupportedSampleRatesCount) count = kSupportedSampleRatesCount;
                    AudioStreamRangedDescription* descs = (AudioStreamRangedDescription*)outData;
                    for (UInt32 i = 0; i < count; ++i) {
                        RomuEq_FillStreamBasicDescription(&descs[i].mFormat, kSupportedSampleRates[i]);
                        descs[i].mSampleRateRange.mMinimum = kSupportedSampleRates[i];
                        descs[i].mSampleRateRange.mMaximum = kSupportedSampleRates[i];
                    }
                    *outDataSize = count * sizeof(AudioStreamRangedDescription);
                    return kAudioHardwareNoError;
                }

                default:
                    return kAudioHardwareUnknownPropertyError;
            }

        // --------------------------------------------------------------------
        // Volume Control Object (ID 5)
        // --------------------------------------------------------------------
        case kObjectID_Volume_Output:
            switch (inAddress->mSelector) {
                case kAudioObjectPropertyBaseClass:
                    if (inDataSize < sizeof(AudioClassID)) return kAudioHardwareBadPropertySizeError;
                    *((AudioClassID*)outData) = kAudioLevelControlClassID;
                    *outDataSize = sizeof(AudioClassID);
                    return kAudioHardwareNoError;

                case kAudioObjectPropertyClass:
                    if (inDataSize < sizeof(AudioClassID)) return kAudioHardwareBadPropertySizeError;
                    *((AudioClassID*)outData) = kAudioVolumeControlClassID;
                    *outDataSize = sizeof(AudioClassID);
                    return kAudioHardwareNoError;

                case kAudioObjectPropertyOwner:
                    if (inDataSize < sizeof(AudioObjectID)) return kAudioHardwareBadPropertySizeError;
                    *((AudioObjectID*)outData) = kObjectID_Device;
                    *outDataSize = sizeof(AudioObjectID);
                    return kAudioHardwareNoError;

                case kAudioObjectPropertyOwnedObjects:
                    *outDataSize = 0;
                    return kAudioHardwareNoError;

                case kAudioControlPropertyScope:
                    if (inDataSize < sizeof(AudioObjectPropertyScope)) return kAudioHardwareBadPropertySizeError;
                    *((AudioObjectPropertyScope*)outData) = kAudioObjectPropertyScopeOutput;
                    *outDataSize = sizeof(AudioObjectPropertyScope);
                    return kAudioHardwareNoError;

                case kAudioControlPropertyElement:
                    if (inDataSize < sizeof(AudioObjectPropertyElement)) return kAudioHardwareBadPropertySizeError;
                    *((AudioObjectPropertyElement*)outData) = kAudioObjectPropertyElementMain;
                    *outDataSize = sizeof(AudioObjectPropertyElement);
                    return kAudioHardwareNoError;

                case kAudioLevelControlPropertyScalarValue:
                    if (inDataSize < sizeof(Float32)) return kAudioHardwareBadPropertySizeError;
                    *((Float32*)outData) = atomic_load_explicit(&gVolumeScalar, memory_order_acquire);
                    *outDataSize = sizeof(Float32);
                    return kAudioHardwareNoError;

                case kAudioLevelControlPropertyDecibelValue: {
                    if (inDataSize < sizeof(Float32)) return kAudioHardwareBadPropertySizeError;
                    Float32 s = atomic_load_explicit(&gVolumeScalar, memory_order_acquire);
                    *((Float32*)outData) = RomuEq_ScalarToDb(s);
                    *outDataSize = sizeof(Float32);
                    return kAudioHardwareNoError;
                }

                case kAudioLevelControlPropertyDecibelRange: {
                    if (inDataSize < sizeof(AudioValueRange)) return kAudioHardwareBadPropertySizeError;
                    AudioValueRange* range = (AudioValueRange*)outData;
                    range->mMinimum = kMinVolumeDb;
                    range->mMaximum = kMaxVolumeDb;
                    *outDataSize = sizeof(AudioValueRange);
                    return kAudioHardwareNoError;
                }

                case kAudioLevelControlPropertyConvertScalarToDecibels:
                    if (inDataSize < sizeof(Float32)) return kAudioHardwareBadPropertySizeError;
                    *((Float32*)outData) = RomuEq_ScalarToDb(*((Float32*)outData));
                    *outDataSize = sizeof(Float32);
                    return kAudioHardwareNoError;

                case kAudioLevelControlPropertyConvertDecibelsToScalar:
                    if (inDataSize < sizeof(Float32)) return kAudioHardwareBadPropertySizeError;
                    *((Float32*)outData) = RomuEq_DbToScalar(*((Float32*)outData));
                    *outDataSize = sizeof(Float32);
                    return kAudioHardwareNoError;

                default:
                    return kAudioHardwareUnknownPropertyError;
            }

        // --------------------------------------------------------------------
        // Mute Control Object (ID 6)
        // --------------------------------------------------------------------
        case kObjectID_Mute_Output:
            switch (inAddress->mSelector) {
                case kAudioObjectPropertyBaseClass:
                    if (inDataSize < sizeof(AudioClassID)) return kAudioHardwareBadPropertySizeError;
                    *((AudioClassID*)outData) = kAudioBooleanControlClassID;
                    *outDataSize = sizeof(AudioClassID);
                    return kAudioHardwareNoError;

                case kAudioObjectPropertyClass:
                    if (inDataSize < sizeof(AudioClassID)) return kAudioHardwareBadPropertySizeError;
                    *((AudioClassID*)outData) = kAudioMuteControlClassID;
                    *outDataSize = sizeof(AudioClassID);
                    return kAudioHardwareNoError;

                case kAudioObjectPropertyOwner:
                    if (inDataSize < sizeof(AudioObjectID)) return kAudioHardwareBadPropertySizeError;
                    *((AudioObjectID*)outData) = kObjectID_Device;
                    *outDataSize = sizeof(AudioObjectID);
                    return kAudioHardwareNoError;

                case kAudioObjectPropertyOwnedObjects:
                    *outDataSize = 0;
                    return kAudioHardwareNoError;

                case kAudioControlPropertyScope:
                    if (inDataSize < sizeof(AudioObjectPropertyScope)) return kAudioHardwareBadPropertySizeError;
                    *((AudioObjectPropertyScope*)outData) = kAudioObjectPropertyScopeOutput;
                    *outDataSize = sizeof(AudioObjectPropertyScope);
                    return kAudioHardwareNoError;

                case kAudioControlPropertyElement:
                    if (inDataSize < sizeof(AudioObjectPropertyElement)) return kAudioHardwareBadPropertySizeError;
                    *((AudioObjectPropertyElement*)outData) = kAudioObjectPropertyElementMain;
                    *outDataSize = sizeof(AudioObjectPropertyElement);
                    return kAudioHardwareNoError;

                case kAudioBooleanControlPropertyValue:
                    if (inDataSize < sizeof(UInt32)) return kAudioHardwareBadPropertySizeError;
                    *((UInt32*)outData) = atomic_load_explicit(&gMuteValue, memory_order_acquire);
                    *outDataSize = sizeof(UInt32);
                    return kAudioHardwareNoError;

                default:
                    return kAudioHardwareUnknownPropertyError;
            }

        default:
            return kAudioHardwareBadObjectError;
    }
}

static OSStatus STDMETHODCALLTYPE RomuEq_SetPropertyData(AudioServerPlugInDriverRef inDriver, AudioObjectID inObjectID, pid_t inClientProcessID, const AudioObjectPropertyAddress* inAddress, UInt32 inQualifierDataSize, const void* inQualifierData, UInt32 inDataSize, const void* inData) {
    (void)inClientProcessID; (void)inQualifierDataSize; (void)inQualifierData;
    if (inDriver != gDriverRef || inAddress == NULL || inData == NULL) {
        return kAudioHardwareBadObjectError;
    }

    switch (inObjectID) {
        case kObjectID_Device:
            if (inAddress->mSelector == kAudioDevicePropertyNominalSampleRate) {
                if (inDataSize < sizeof(Float64)) return kAudioHardwareBadPropertySizeError;
                Float64 newSampleRate = *((const Float64*)inData);
                if (!RomuEq_IsValidSampleRate(newSampleRate)) {
                    return kAudioHardwareIllegalOperationError;
                }
                AudioServerPlugInHostRef host = gPlugInHost;
                if (host != NULL) {
                    host->RequestDeviceConfigurationChange(host, kObjectID_Device, (UInt64)newSampleRate, NULL);
                } else {
                    RomuEq_PerformDeviceConfigurationChange(inDriver, kObjectID_Device, (UInt64)newSampleRate, NULL);
                }
                return kAudioHardwareNoError;
            }
            return kAudioHardwareUnknownPropertyError;

        case kObjectID_Stream_Input:
        case kObjectID_Stream_Output:
            if (inAddress->mSelector == kAudioStreamPropertyVirtualFormat ||
                inAddress->mSelector == kAudioStreamPropertyPhysicalFormat) {
                if (inDataSize < sizeof(AudioStreamBasicDescription)) return kAudioHardwareBadPropertySizeError;
                const AudioStreamBasicDescription* desc = (const AudioStreamBasicDescription*)inData;
                if (!RomuEq_IsValidSampleRate(desc->mSampleRate)) {
                    return kAudioHardwareIllegalOperationError;
                }
                AudioServerPlugInHostRef host = gPlugInHost;
                if (host != NULL) {
                    host->RequestDeviceConfigurationChange(host, kObjectID_Device, (UInt64)desc->mSampleRate, NULL);
                } else {
                    RomuEq_PerformDeviceConfigurationChange(inDriver, kObjectID_Device, (UInt64)desc->mSampleRate, NULL);
                }
                return kAudioHardwareNoError;
            }
            return kAudioHardwareUnknownPropertyError;

        case kObjectID_Volume_Output:
            if (inAddress->mSelector == kAudioLevelControlPropertyScalarValue) {
                if (inDataSize < sizeof(Float32)) return kAudioHardwareBadPropertySizeError;
                Float32 val = *((const Float32*)inData);
                if (isnan(val) || val < 0.0f) val = 0.0f;
                else if (val > 1.0f) val = 1.0f;
                atomic_store_explicit(&gVolumeScalar, val, memory_order_release);
                if (gPlugInHost != NULL) {
                    AudioObjectPropertyAddress addrs[] = {
                        { kAudioLevelControlPropertyScalarValue,  kAudioObjectPropertyScopeGlobal, kAudioObjectPropertyElementMain },
                        { kAudioLevelControlPropertyDecibelValue, kAudioObjectPropertyScopeGlobal, kAudioObjectPropertyElementMain },
                    };
                    gPlugInHost->PropertiesChanged(gPlugInHost, kObjectID_Volume_Output, 2, addrs);
                }
                return kAudioHardwareNoError;
            } else if (inAddress->mSelector == kAudioLevelControlPropertyDecibelValue) {
                if (inDataSize < sizeof(Float32)) return kAudioHardwareBadPropertySizeError;
                Float32 db = *((const Float32*)inData);
                if (isnan(db)) db = kMinVolumeDb;
                Float32 val = RomuEq_DbToScalar(db);
                atomic_store_explicit(&gVolumeScalar, val, memory_order_release);
                if (gPlugInHost != NULL) {
                    AudioObjectPropertyAddress addrs[] = {
                        { kAudioLevelControlPropertyScalarValue,  kAudioObjectPropertyScopeGlobal, kAudioObjectPropertyElementMain },
                        { kAudioLevelControlPropertyDecibelValue, kAudioObjectPropertyScopeGlobal, kAudioObjectPropertyElementMain },
                    };
                    gPlugInHost->PropertiesChanged(gPlugInHost, kObjectID_Volume_Output, 2, addrs);
                }
                return kAudioHardwareNoError;
            }
            return kAudioHardwareUnknownPropertyError;

        case kObjectID_Mute_Output:
            if (inAddress->mSelector == kAudioBooleanControlPropertyValue) {
                if (inDataSize < sizeof(UInt32)) return kAudioHardwareBadPropertySizeError;
                UInt32 val = (*((const UInt32*)inData) != 0) ? 1 : 0;
                atomic_store_explicit(&gMuteValue, val, memory_order_release);
                if (gPlugInHost != NULL) {
                    AudioObjectPropertyAddress addr = {
                        kAudioBooleanControlPropertyValue,
                        kAudioObjectPropertyScopeGlobal,
                        kAudioObjectPropertyElementMain
                    };
                    gPlugInHost->PropertiesChanged(gPlugInHost, kObjectID_Mute_Output, 1, &addr);
                }
                return kAudioHardwareNoError;
            }
            return kAudioHardwareUnknownPropertyError;

        default:
            return kAudioHardwareBadObjectError;
    }
}

// ============================================================================
// Real-Time IO & Clock Operations
// ============================================================================

static OSStatus STDMETHODCALLTYPE RomuEq_StartIO(AudioServerPlugInDriverRef inDriver, AudioObjectID inDeviceObjectID, UInt32 inClientID) {
    (void)inClientID;
    if (inDriver != gDriverRef || inDeviceObjectID != kObjectID_Device) {
        return kAudioHardwareBadObjectError;
    }
    pthread_mutex_lock(&gStateMutex);
    UInt32 prev = atomic_fetch_add_explicit(&gIOClientCount, 1, memory_order_acq_rel);
    if (prev == 0) {
        memset(gRingBuffer, 0, sizeof(gRingBuffer));
        for (UInt32 i = 0; i < kRingBufferFrames; ++i) {
            atomic_store_explicit(&gFrameStamp[i], 0, memory_order_relaxed);
        }
        Float64 sr = atomic_load_explicit(&gSampleRate, memory_order_acquire);
        RomuEq_UpdateClock(sr);
    }
    pthread_mutex_unlock(&gStateMutex);
    return kAudioHardwareNoError;
}

static OSStatus STDMETHODCALLTYPE RomuEq_StopIO(AudioServerPlugInDriverRef inDriver, AudioObjectID inDeviceObjectID, UInt32 inClientID) {
    (void)inClientID;
    if (inDriver != gDriverRef || inDeviceObjectID != kObjectID_Device) {
        return kAudioHardwareBadObjectError;
    }
    pthread_mutex_lock(&gStateMutex);
    UInt32 prev = atomic_load_explicit(&gIOClientCount, memory_order_acquire);
    if (prev > 0) {
        if (prev == 1) {
            memset(gRingBuffer, 0, sizeof(gRingBuffer));
            for (UInt32 i = 0; i < kRingBufferFrames; ++i) {
                atomic_store_explicit(&gFrameStamp[i], 0, memory_order_relaxed);
            }
        }
        atomic_fetch_sub_explicit(&gIOClientCount, 1, memory_order_acq_rel);
    }
    pthread_mutex_unlock(&gStateMutex);
    return kAudioHardwareNoError;
}

static OSStatus STDMETHODCALLTYPE RomuEq_GetZeroTimeStamp(AudioServerPlugInDriverRef inDriver, AudioObjectID inDeviceObjectID, UInt32 inClientID, Float64* outSampleTime, UInt64* outHostTime, UInt64* outSeed) {
    (void)inClientID;
    if (inDriver != gDriverRef || inDeviceObjectID != kObjectID_Device) {
        return kAudioHardwareBadObjectError;
    }
    if (outSampleTime == NULL || outHostTime == NULL || outSeed == NULL) {
        return kAudioHardwareIllegalOperationError;
    }

    UInt64 now = mach_absolute_time();
    UInt64 anchor = 0;
    Float64 ticksPerFrame = 0.0;
    UInt64 seed = 0;

    // Lock-free sequence lock read (real-time safe, guarantees atomic snapshot of clock parameters)
    UInt32 seq_start, seq_end;
    do {
        seq_start = atomic_load_explicit(&gClockSeq, memory_order_acquire);
        if (seq_start & 1) {
            continue; // Writer active, retry
        }
        anchor = gAnchorHostTime;
        ticksPerFrame = gHostTicksPerFrame;
        seed = gClockSeed;
        seq_end = atomic_load_explicit(&gClockSeq, memory_order_acquire);
    } while (seq_start != seq_end || (seq_start & 1));

    if (ticksPerFrame <= 0.0) {
        *outSampleTime = 0.0;
        *outHostTime = now;
        *outSeed = seed;
        return kAudioHardwareNoError;
    }

    Float64 ticksPerRing = ticksPerFrame * (Float64)kRingBufferFrames;
    UInt64 elapsedTicks = (now >= anchor) ? (now - anchor) : 0;
    UInt64 completedPeriods = (UInt64)((Float64)elapsedTicks / ticksPerRing);

    UInt64 periodSampleTime = completedPeriods * (UInt64)kRingBufferFrames;
    UInt64 periodHostTime   = anchor + (UInt64)((Float64)periodSampleTime * ticksPerFrame);

    *outSampleTime = (Float64)periodSampleTime;
    *outHostTime   = periodHostTime;
    *outSeed       = seed;
    return kAudioHardwareNoError;
}

static OSStatus STDMETHODCALLTYPE RomuEq_WillDoIOOperation(AudioServerPlugInDriverRef inDriver, AudioObjectID inDeviceObjectID, UInt32 inClientID, UInt32 inOperationID, Boolean* outWillDo, Boolean* outWillDoInPlace) {
    (void)inClientID;
    if (inDriver != gDriverRef || inDeviceObjectID != kObjectID_Device) {
        return kAudioHardwareBadObjectError;
    }
    if (outWillDo == NULL || outWillDoInPlace == NULL) {
        return kAudioHardwareIllegalOperationError;
    }

    switch (inOperationID) {
        case kAudioServerPlugInIOOperationWriteMix:
        case kAudioServerPlugInIOOperationReadInput:
            *outWillDo = true;
            *outWillDoInPlace = true;
            break;
        default:
            *outWillDo = false;
            *outWillDoInPlace = true;
            break;
    }
    return kAudioHardwareNoError;
}

static OSStatus STDMETHODCALLTYPE RomuEq_BeginIOOperation(AudioServerPlugInDriverRef inDriver, AudioObjectID inDeviceObjectID, UInt32 inClientID, UInt32 inOperationID, UInt32 inIOBufferFrameSize, const AudioServerPlugInIOCycleInfo* inIOCycleInfo) {
    (void)inDriver; (void)inDeviceObjectID; (void)inClientID; (void)inOperationID; (void)inIOBufferFrameSize; (void)inIOCycleInfo;
    return kAudioHardwareNoError;
}

static OSStatus STDMETHODCALLTYPE RomuEq_DoIOOperation(AudioServerPlugInDriverRef inDriver, AudioObjectID inDeviceObjectID, AudioObjectID inStreamObjectID, UInt32 inClientID, UInt32 inOperationID, UInt32 inIOBufferFrameSize, const AudioServerPlugInIOCycleInfo* inIOCycleInfo, void* ioMainBuffer, void* ioSecondaryBuffer) {
    (void)inStreamObjectID; (void)inClientID; (void)ioSecondaryBuffer;
    if (inDriver != gDriverRef || inDeviceObjectID != kObjectID_Device) {
        return kAudioHardwareBadObjectError;
    }
    if (inIOCycleInfo == NULL || ioMainBuffer == NULL) {
        return kAudioHardwareIllegalOperationError;
    }

    if (inOperationID == kAudioServerPlugInIOOperationWriteMix) {
        const float* src = (const float*)ioMainBuffer;
        UInt64 startSample = (UInt64)inIOCycleInfo->mOutputTime.mSampleTime;
        UInt32 muted = atomic_load_explicit(&gMuteValue, memory_order_relaxed);
        Float32 gain = muted ? 0.0f : atomic_load_explicit(&gVolumeScalar, memory_order_relaxed);
        if (isnan(gain) || gain < 0.0f) gain = 0.0f;
        else if (gain > 1.0f) gain = 1.0f;

        for (UInt32 i = 0; i < inIOBufferFrameSize; ++i) {
            UInt64 sampleTime = startSample + (UInt64)i;
            UInt32 ringIdx = (UInt32)(sampleTime & kRingBufferMask);
            gRingBuffer[ringIdx * 2]     = src[i * 2]     * gain;
            gRingBuffer[ringIdx * 2 + 1] = src[i * 2 + 1] * gain;
            atomic_store_explicit(&gFrameStamp[ringIdx], sampleTime + 1, memory_order_release);
        }
    } else if (inOperationID == kAudioServerPlugInIOOperationReadInput) {
        float* dst = (float*)ioMainBuffer;
        UInt64 startSample = (UInt64)inIOCycleInfo->mInputTime.mSampleTime;

        for (UInt32 i = 0; i < inIOBufferFrameSize; ++i) {
            UInt64 sampleTime = startSample + (UInt64)i;
            UInt32 ringIdx = (UInt32)(sampleTime & kRingBufferMask);
            UInt64 stamp = atomic_load_explicit(&gFrameStamp[ringIdx], memory_order_acquire);

            if (stamp == (sampleTime + 1)) {
                dst[i * 2]     = gRingBuffer[ringIdx * 2];
                dst[i * 2 + 1] = gRingBuffer[ringIdx * 2 + 1];
                // Cleanly zero out consumed frame so silence is maintained if playback pauses
                gRingBuffer[ringIdx * 2]     = 0.0f;
                gRingBuffer[ringIdx * 2 + 1] = 0.0f;
                atomic_store_explicit(&gFrameStamp[ringIdx], 0, memory_order_release);
            } else {
                dst[i * 2]     = 0.0f;
                dst[i * 2 + 1] = 0.0f;
            }
        }
    }

    return kAudioHardwareNoError;
}

static OSStatus STDMETHODCALLTYPE RomuEq_EndIOOperation(AudioServerPlugInDriverRef inDriver, AudioObjectID inDeviceObjectID, UInt32 inClientID, UInt32 inOperationID, UInt32 inIOBufferFrameSize, const AudioServerPlugInIOCycleInfo* inIOCycleInfo) {
    (void)inDriver; (void)inDeviceObjectID; (void)inClientID; (void)inOperationID; (void)inIOBufferFrameSize; (void)inIOCycleInfo;
    return kAudioHardwareNoError;
}

