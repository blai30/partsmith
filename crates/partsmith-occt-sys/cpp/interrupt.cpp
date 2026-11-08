// The progress indicator of the calls that poll an interrupt, the shim's own polls, the web
// core's cancel word and the test wait (kernel spec section 3.13).
#include <chrono>
#include <string>
#include <thread>

#include "partsmith-occt-sys/cpp/internal.h"

#ifdef __EMSCRIPTEN__
#include <emscripten/em_js.h>

// The web page's cancel word (kernel spec section 3.13 item 7): the id of the request the page
// asks to stop, as a double, or 0 when the worker gave the module no word. It never throws.
EM_JS(double, partsmith_cancel_word, (), {
  var word = Module['partsmithCancelWord'];
  if (word instanceof BigInt64Array && word.length >= 1) {
    return Number(Atomics.load(word, 0));
  }
  return 0;
});
#endif

namespace partsmith {
namespace occt {

using namespace detail;

namespace {

// Throws Partsmith_Canceled with a message naming the step that saw the request.
[[noreturn]] void raise_canceled(const std::string &message) {
  throw Partsmith_Canceled(message.c_str());
}

}  // namespace

Standard_Boolean Interruption::UserBreak() {
  if (!answered_) {
    answered_ = interrupt_.is_requested();
  }
  return answered_;
}

namespace detail {

Polls::Polls(const Handle(Interruption) &indicator)
    : indicator_(indicator),
      scope_(Message_ProgressIndicator::Start(indicator_), nullptr, 1, Standard_True) {}

void Polls::before(const char *step) {
  if (!indicator_.IsNull() && indicator_->UserBreak()) {
    raise_canceled(std::string("stopped by its interrupt before ") + step);
  }
}

void Polls::stop_if_answered(const char *algorithm) const {
  if (!indicator_.IsNull() && indicator_->answered()) {
    raise_canceled(std::string(algorithm) + " stopped by its interrupt");
  }
}

}  // namespace detail

void wait_for_interrupt(const Interrupt &interrupt, double limit) {
  const Handle(Interruption) indicator = new Interruption(interrupt);
  const Message_ProgressScope scope(indicator->Start(), nullptr, 1, Standard_True);
  const auto start = std::chrono::steady_clock::now();
  // The first poll comes before the limit. A request before the call always stops it.
  while (!scope.UserBreak()) {
    const std::chrono::duration<double, std::milli> waited =
        std::chrono::steady_clock::now() - start;
    if (waited.count() >= limit) {
      return;
    }
    std::this_thread::sleep_for(std::chrono::milliseconds(1));
  }
  raise_canceled("wait_for_interrupt stopped by its interrupt");
}

#ifdef __EMSCRIPTEN__
double page_cancel_word() { return partsmith_cancel_word(); }
#endif

}  // namespace occt
}  // namespace partsmith
