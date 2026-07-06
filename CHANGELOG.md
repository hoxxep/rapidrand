# Changelog

## 0.2.0 (20260704)

### Changes
- **Breaking: renamed `rapidrng` to `rapidrand` and `RapidRng` to `RapidRand`** for consistency with the crate name (and the wyrand-family `WyRand` naming convention).
- **Breaking: switched `rapidrand` and `RapidRand` to a [`wyranda`](https://github.com/wangyi-fudan/wyhash/issues/156) construction** proposed by @vigna and @reinerp ([analysis](https://github.com/wangyi-fudan/wyhash/issues/130#issuecomment-4835746792)). It feeds the output multiply from two consecutive counter states instead of one, raising output-space coverage from ~39.3% to ~63.2% at identical speed. See the new "Quality: wyranda vs wyrand" section in the README. **This changes the PRNG output values from v0.1.**
- **Breaking:** removed the w1rand-based `rapidrng_single` function for its statistical weaknesses.

## 0.1.2 (20260703)

### Changes
- Simplify the README benchmark table.

## 0.1.1 (20260703)

### Changes
- Improved documentation and examples.
- Separated tests out of the main `src/` to reduce SLoC.

## 0.1.0 (20260702)

Initial release and separation from the [rapidhash](https://github.com/hoxxep/rapidhash) crate.
