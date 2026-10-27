// Double-double arithmetic for the shim's copy of Delabella (kernel spec section 3.7, item 2).
//
// A value is the unevaluated sum of two doubles, `high + low` with `high` the double nearest to
// it, so about 106 bits. Every operation uses double addition, subtraction and multiplication
// alone (Knuth's two-sum, Dekker's product), never a fused multiply-add or a library call, so it
// rounds the same way on every platform. A compiler that fused a multiplication and an addition
// here would change the low parts: build.rs compiles the shim with -ffp-contract=off, and MSVC
// fuses none under its default /fp:precise (kernel spec section 3.12, item 5).
#pragma once

namespace partsmith {
namespace occt {

class DoubleDouble {
 public:
  // Uninitialized, like the long double it replaces, so arrays of Delabella's vertices and faces
  // can still be allocated with malloc.
  DoubleDouble() = default;

  // Implicit, so Delabella's casts and its comparisons with 0 read as they do with long double.
  constexpr DoubleDouble(double value) : high_(value), low_(0.0) {}

  constexpr double high() const { return high_; }
  constexpr double low() const { return low_; }

  friend constexpr DoubleDouble operator+(const DoubleDouble &first, const DoubleDouble &second) {
    const DoubleDouble highs = two_sum(first.high_, second.high_);
    const DoubleDouble lows = two_sum(first.low_, second.low_);
    const DoubleDouble partial = fast_two_sum(highs.high_, highs.low_ + lows.high_);
    return fast_two_sum(partial.high_, partial.low_ + lows.low_);
  }

  friend constexpr DoubleDouble operator-(const DoubleDouble &first, const DoubleDouble &second) {
    return first + DoubleDouble(-second.high_, -second.low_);
  }

  friend constexpr DoubleDouble operator*(const DoubleDouble &first, double second) {
    const DoubleDouble product = two_product(first.high_, second);
    return fast_two_sum(product.high_, product.low_ + first.low_ * second);
  }

  friend constexpr DoubleDouble operator*(const DoubleDouble &first, const DoubleDouble &second) {
    const DoubleDouble product = two_product(first.high_, second.high_);
    const double cross = first.high_ * second.low_ + first.low_ * second.high_;
    return fast_two_sum(product.high_, product.low_ + cross);
  }

  // Every value is normalized (`high` is the double nearest to the sum), so values compare by
  // their high parts first and then by their low parts.
  friend constexpr bool operator==(const DoubleDouble &first, const DoubleDouble &second) {
    return first.high_ == second.high_ && first.low_ == second.low_;
  }

  friend constexpr bool operator<(const DoubleDouble &first, const DoubleDouble &second) {
    return first.high_ < second.high_ || (first.high_ == second.high_ && first.low_ < second.low_);
  }

  friend constexpr bool operator>(const DoubleDouble &first, const DoubleDouble &second) {
    return second < first;
  }

  friend constexpr bool operator<=(const DoubleDouble &first, const DoubleDouble &second) {
    return !(second < first);
  }

  // The exact sum of two doubles (Knuth's two-sum).
  static constexpr DoubleDouble two_sum(double first, double second) {
    const double sum = first + second;
    const double second_part = sum - first;
    const double error = (first - (sum - second_part)) + (second - second_part);
    return DoubleDouble(sum, error);
  }

  // The exact product of two doubles (Dekker's product, with Veltkamp's split into 26-bit
  // halves).
  static constexpr DoubleDouble two_product(double first, double second) {
    const DoubleDouble first_halves = split(first);
    const DoubleDouble second_halves = split(second);
    const double product = first * second;
    const double error = ((first_halves.high_ * second_halves.high_ - product) +
                          first_halves.high_ * second_halves.low_ +
                          first_halves.low_ * second_halves.high_) +
                         first_halves.low_ * second_halves.low_;
    return DoubleDouble(product, error);
  }

 private:
  constexpr DoubleDouble(double high, double low) : high_(high), low_(low) {}

  // The exact sum of two doubles when the first is not smaller in magnitude than the second.
  static constexpr DoubleDouble fast_two_sum(double larger, double smaller) {
    const double sum = larger + smaller;
    return DoubleDouble(sum, smaller - (sum - larger));
  }

  // A double as the sum of two doubles of at most 26 significant bits each, for values far below
  // 2^996 in magnitude, as Delabella's coordinates in the unit square and their products are.
  static constexpr DoubleDouble split(double value) {
    // 2^27 + 1.
    const double scaled = 134217729.0 * value;
    const double high = scaled - (scaled - value);
    return DoubleDouble(high, value - high);
  }

  double high_;
  double low_;
};

// The algorithm checked at compile time. This checks the arithmetic as written, not the code the
// compiler emits at run time, where kernel spec test 137 and the flags of build.rs guard it.
static_assert(DoubleDouble::two_sum(1.0, 0x1p-60).high() == 1.0 &&
                  DoubleDouble::two_sum(1.0, 0x1p-60).low() == 0x1p-60,
              "two-sum keeps the low part of 1 + 2^-60");
static_assert(DoubleDouble::two_product(1.0 + 0x1p-30, 1.0 + 0x1p-30).high() == 1.0 + 0x1p-29 &&
                  DoubleDouble::two_product(1.0 + 0x1p-30, 1.0 + 0x1p-30).low() == 0x1p-60,
              "Dekker's product keeps the low part of (1 + 2^-30)^2");
static_assert((DoubleDouble(1.0 + 0x1p-30) * (1.0 + 0x1p-30)).low() == 0x1p-60 &&
                  (DoubleDouble(1.0 + 0x1p-30) * DoubleDouble(1.0 + 0x1p-30)).low() == 0x1p-60,
              "products keep the bits that a double rounds away");
static_assert(DoubleDouble(1.0) - 0x1p-60 < DoubleDouble(1.0) &&
                  DoubleDouble(1.0) < DoubleDouble(1.0) + 0x1p-60,
              "values that differ only in their low parts compare by them");

}  // namespace occt
}  // namespace partsmith
