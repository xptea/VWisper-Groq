#include <CoreGraphics/CoreGraphics.h>
#include <CoreFoundation/CoreFoundation.h>

typedef void (*vwisper_fn_callback_t)(void);

static CFMachPortRef g_event_tap = NULL;
static CFRunLoopSourceRef g_run_loop_source = NULL;
static CFRunLoopRef g_run_loop = NULL;
static vwisper_fn_callback_t g_on_down = NULL;
static vwisper_fn_callback_t g_on_up = NULL;
static int g_last_fn_state = 0;
static int g_initialized = 0;

static CGEventRef vwisper_event_callback(
    CGEventTapProxy proxy,
    CGEventType type,
    CGEventRef event,
    void * userInfo)
{
  (void)proxy;
  (void)userInfo;
  if (type == kCGEventTapDisabledByTimeout || type == kCGEventTapDisabledByUserInput) {
    if (g_event_tap) {
      CGEventTapEnable(g_event_tap, true);
    }
    return event;
  }

  if (type == kCGEventFlagsChanged) {
    CGEventFlags flags = CGEventGetFlags(event);
#ifdef kCGEventFlagMaskSecondaryFn
    int fn_down = (flags & kCGEventFlagMaskSecondaryFn) == kCGEventFlagMaskSecondaryFn;
#else
    const uint64_t VWISPER_FN_MASK = 0x00800000ULL;
    int fn_down = (flags & VWISPER_FN_MASK) == VWISPER_FN_MASK;
#endif

    if (!g_initialized) {
      g_last_fn_state = fn_down;
      g_initialized = 1;
      return event;
    }

    if (fn_down != g_last_fn_state) {
      g_last_fn_state = fn_down;
      if (fn_down) {
        if (g_on_down) g_on_down();
      } else {
        if (g_on_up) g_on_up();
      }
    }
  }

  return event;
}

void vwisper_start_fn_monitor(vwisper_fn_callback_t on_down, vwisper_fn_callback_t on_up) {
  g_on_down = on_down;
  g_on_up = on_up;

  if (g_event_tap != NULL) {
    return;
  }

  CGEventMask mask = (1ULL << kCGEventFlagsChanged);
  g_event_tap = CGEventTapCreate(kCGHIDEventTap,
                                 kCGHeadInsertEventTap,
                                 kCGEventTapOptionListenOnly,
                                 mask,
                                 vwisper_event_callback,
                                 NULL);
  if (!g_event_tap) {
    return;
  }

  g_run_loop_source = CFMachPortCreateRunLoopSource(kCFAllocatorDefault, g_event_tap, 0);
  g_run_loop = CFRunLoopGetCurrent();
  CFRunLoopAddSource(g_run_loop, g_run_loop_source, kCFRunLoopCommonModes);
  CGEventTapEnable(g_event_tap, true);

  CGEventFlags flags = CGEventSourceFlagsState(kCGEventSourceStateCombinedSessionState);
#ifdef kCGEventFlagMaskSecondaryFn
  int fn_down = (flags & kCGEventFlagMaskSecondaryFn) == kCGEventFlagMaskSecondaryFn;
#else
  const uint64_t VWISPER_FN_MASK = 0x00800000ULL;
  int fn_down = (flags & VWISPER_FN_MASK) == VWISPER_FN_MASK;
#endif
  g_last_fn_state = fn_down;
  g_initialized = 1;

  CFRunLoopRun();
}

void vwisper_stop_fn_monitor(void) {
  if (g_run_loop) {
    CFRunLoopStop(g_run_loop);
  }
  if (g_event_tap) {
    CGEventTapEnable(g_event_tap, false);
    CFMachPortInvalidate(g_event_tap);
    CFRelease(g_event_tap);
    g_event_tap = NULL;
  }
  if (g_run_loop_source) {
    CFRelease(g_run_loop_source);
    g_run_loop_source = NULL;
  }
  g_run_loop = NULL;
  g_on_down = NULL;
  g_on_up = NULL;
  g_last_fn_state = 0;
  g_initialized = 0;
}