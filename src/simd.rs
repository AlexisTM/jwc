//! SIMD-accelerated byte scans used by both parsers.
//!
//! Public API is three scalar-looking functions. Each dispatches to AVX2 →
//! SSE4.2 → scalar at runtime; dispatch is cached via a function pointer
//! stored in an atomic, so every subsequent call is a single indirect jump
//! plus the SIMD body.
//!
//! `x86_64` without SSE4.2 / AVX2, or when `simd` feature is disabled, uses
//! the scalar fallback. The fallback is also what other targets (aarch64,
//! wasm, …) get; if / when `std::arch::aarch64` becomes worth targeting,
//! add a branch here.

#![allow(clippy::missing_safety_doc)]

/// Return the first index `>= from` where `bytes[i]` is `"` or `\\`, or
/// `bytes.len()` if none exists.
#[inline]
pub fn find_string_end(bytes: &[u8], from: usize) -> usize {
    // Short-remainder bypass: scalar is cheaper than dispatch + SIMD setup
    // when there's less than one vector lane to scan.
    if bytes.len().saturating_sub(from) < 16 {
        return scalar::find_string_end(bytes, from);
    }
    dispatch::find_string_end(bytes, from)
}

/// Return the first index `>= from` where `bytes[i]` is NOT one of
/// `{' ', '\t', '\n', '\r'}`, or `bytes.len()`.
#[inline]
pub fn skip_ws(bytes: &[u8], from: usize) -> usize {
    if bytes.len().saturating_sub(from) < 16 {
        return scalar::skip_ws(bytes, from);
    }
    dispatch::skip_ws(bytes, from)
}

/// Return the first index `>= from` where `bytes[i] == b'\n'`, or
/// `bytes.len()`.
#[inline]
pub fn find_newline(bytes: &[u8], from: usize) -> usize {
    if bytes.len().saturating_sub(from) < 16 {
        return scalar::find_newline(bytes, from);
    }
    dispatch::find_newline(bytes, from)
}

/// Return the first index `>= from` where `bytes[i]` is one of
/// `{ } [ ] , : " /` or `bytes.len()`. Currently unused by the live
/// parsers (bitmap stage 1 handles classification in bulk), but kept
/// for scalar fallback and future use. `/` is included so JSONC
/// comment detection can stop at one.
#[inline]
#[allow(dead_code)]
pub fn find_structural(bytes: &[u8], from: usize) -> usize {
    if bytes.len().saturating_sub(from) < 16 {
        return scalar::find_structural(bytes, from);
    }
    dispatch::find_structural(bytes, from)
}

// ---------------------------------------------------------------------------
// Scalar fallback (always available; used as the default before detection).
// ---------------------------------------------------------------------------

mod scalar {
    #[inline]
    pub fn find_string_end(bytes: &[u8], from: usize) -> usize {
        let mut i = from;
        while i < bytes.len() {
            let b = unsafe { *bytes.get_unchecked(i) };
            if b == b'"' || b == b'\\' {
                return i;
            }
            i += 1;
        }
        bytes.len()
    }

    #[inline]
    pub fn skip_ws(bytes: &[u8], from: usize) -> usize {
        let mut i = from;
        while i < bytes.len() {
            let b = unsafe { *bytes.get_unchecked(i) };
            if b == b' ' || b == b'\n' || b == b'\r' || b == b'\t' {
                i += 1;
            } else {
                break;
            }
        }
        i
    }

    #[inline]
    pub fn find_newline(bytes: &[u8], from: usize) -> usize {
        let mut i = from;
        while i < bytes.len() {
            if unsafe { *bytes.get_unchecked(i) } == b'\n' {
                return i;
            }
            i += 1;
        }
        bytes.len()
    }

    #[inline]
    pub fn find_structural(bytes: &[u8], from: usize) -> usize {
        let mut i = from;
        while i < bytes.len() {
            let b = unsafe { *bytes.get_unchecked(i) };
            if matches!(b, b'{' | b'}' | b'[' | b']' | b',' | b':' | b'"' | b'/') {
                return i;
            }
            i += 1;
        }
        bytes.len()
    }
}

// ---------------------------------------------------------------------------
// x86_64 SIMD impls.
// ---------------------------------------------------------------------------

#[cfg(all(feature = "simd", target_arch = "x86_64"))]
mod x86 {
    use std::arch::x86_64::*;

    // ----- SSE4.2 -----

    #[target_feature(enable = "sse4.2")]
    pub unsafe fn find_string_end_sse42(bytes: &[u8], from: usize) -> usize {
        unsafe {
            let quote = _mm_set1_epi8(b'"' as i8);
            let bslash = _mm_set1_epi8(b'\\' as i8);
            let mut i = from;
            while i + 16 <= bytes.len() {
                let v = _mm_loadu_si128(bytes.as_ptr().add(i) as *const __m128i);
                let eq_q = _mm_cmpeq_epi8(v, quote);
                let eq_b = _mm_cmpeq_epi8(v, bslash);
                let mask = _mm_movemask_epi8(_mm_or_si128(eq_q, eq_b)) as u32;
                if mask != 0 {
                    return i + mask.trailing_zeros() as usize;
                }
                i += 16;
            }
            super::scalar::find_string_end(bytes, i)
        }
    }

    #[target_feature(enable = "sse4.2")]
    pub unsafe fn skip_ws_sse42(bytes: &[u8], from: usize) -> usize {
        unsafe {
            let sp = _mm_set1_epi8(b' ' as i8);
            let tab = _mm_set1_epi8(b'\t' as i8);
            let nl = _mm_set1_epi8(b'\n' as i8);
            let cr = _mm_set1_epi8(b'\r' as i8);
            let mut i = from;
            while i + 16 <= bytes.len() {
                let v = _mm_loadu_si128(bytes.as_ptr().add(i) as *const __m128i);
                let m = _mm_or_si128(
                    _mm_or_si128(_mm_cmpeq_epi8(v, sp), _mm_cmpeq_epi8(v, tab)),
                    _mm_or_si128(_mm_cmpeq_epi8(v, nl), _mm_cmpeq_epi8(v, cr)),
                );
                // bit=1 where byte is whitespace; we want the first NON-ws byte.
                let mask = (!_mm_movemask_epi8(m) as u32) & 0xFFFF;
                if mask != 0 {
                    return i + mask.trailing_zeros() as usize;
                }
                i += 16;
            }
            super::scalar::skip_ws(bytes, i)
        }
    }

    #[target_feature(enable = "sse4.2")]
    pub unsafe fn find_newline_sse42(bytes: &[u8], from: usize) -> usize {
        unsafe {
            let nl = _mm_set1_epi8(b'\n' as i8);
            let mut i = from;
            while i + 16 <= bytes.len() {
                let v = _mm_loadu_si128(bytes.as_ptr().add(i) as *const __m128i);
                let mask = _mm_movemask_epi8(_mm_cmpeq_epi8(v, nl)) as u32;
                if mask != 0 {
                    return i + mask.trailing_zeros() as usize;
                }
                i += 16;
            }
            super::scalar::find_newline(bytes, i)
        }
    }

    /// Find any of `{ } [ ] , : " /` via 8 cmpeq + OR reduction.
    #[target_feature(enable = "sse4.2")]
    pub unsafe fn find_structural_sse42(bytes: &[u8], from: usize) -> usize {
        unsafe {
            let c_lbrace = _mm_set1_epi8(b'{' as i8);
            let c_rbrace = _mm_set1_epi8(b'}' as i8);
            let c_lbracket = _mm_set1_epi8(b'[' as i8);
            let c_rbracket = _mm_set1_epi8(b']' as i8);
            let c_comma = _mm_set1_epi8(b',' as i8);
            let c_colon = _mm_set1_epi8(b':' as i8);
            let c_quote = _mm_set1_epi8(b'"' as i8);
            let c_slash = _mm_set1_epi8(b'/' as i8);
            let mut i = from;
            while i + 16 <= bytes.len() {
                let v = _mm_loadu_si128(bytes.as_ptr().add(i) as *const __m128i);
                let g1 = _mm_or_si128(
                    _mm_or_si128(_mm_cmpeq_epi8(v, c_lbrace), _mm_cmpeq_epi8(v, c_rbrace)),
                    _mm_or_si128(_mm_cmpeq_epi8(v, c_lbracket), _mm_cmpeq_epi8(v, c_rbracket)),
                );
                let g2 = _mm_or_si128(
                    _mm_or_si128(_mm_cmpeq_epi8(v, c_comma), _mm_cmpeq_epi8(v, c_colon)),
                    _mm_or_si128(_mm_cmpeq_epi8(v, c_quote), _mm_cmpeq_epi8(v, c_slash)),
                );
                let mask = _mm_movemask_epi8(_mm_or_si128(g1, g2)) as u32;
                if mask != 0 {
                    return i + mask.trailing_zeros() as usize;
                }
                i += 16;
            }
            super::scalar::find_structural(bytes, i)
        }
    }

    // ----- AVX2 (32-byte lanes) -----

    #[target_feature(enable = "avx2")]
    pub unsafe fn find_string_end_avx2(bytes: &[u8], from: usize) -> usize {
        unsafe {
            let quote = _mm256_set1_epi8(b'"' as i8);
            let bslash = _mm256_set1_epi8(b'\\' as i8);
            let mut i = from;
            while i + 32 <= bytes.len() {
                let v = _mm256_loadu_si256(bytes.as_ptr().add(i) as *const __m256i);
                let eq_q = _mm256_cmpeq_epi8(v, quote);
                let eq_b = _mm256_cmpeq_epi8(v, bslash);
                let mask = _mm256_movemask_epi8(_mm256_or_si256(eq_q, eq_b)) as u32;
                if mask != 0 {
                    return i + mask.trailing_zeros() as usize;
                }
                i += 32;
            }
            find_string_end_sse42(bytes, i)
        }
    }

    #[target_feature(enable = "avx2")]
    pub unsafe fn skip_ws_avx2(bytes: &[u8], from: usize) -> usize {
        unsafe {
            let sp = _mm256_set1_epi8(b' ' as i8);
            let tab = _mm256_set1_epi8(b'\t' as i8);
            let nl = _mm256_set1_epi8(b'\n' as i8);
            let cr = _mm256_set1_epi8(b'\r' as i8);
            let mut i = from;
            while i + 32 <= bytes.len() {
                let v = _mm256_loadu_si256(bytes.as_ptr().add(i) as *const __m256i);
                let m = _mm256_or_si256(
                    _mm256_or_si256(_mm256_cmpeq_epi8(v, sp), _mm256_cmpeq_epi8(v, tab)),
                    _mm256_or_si256(_mm256_cmpeq_epi8(v, nl), _mm256_cmpeq_epi8(v, cr)),
                );
                let mask = !_mm256_movemask_epi8(m) as u32;
                if mask != 0 {
                    return i + mask.trailing_zeros() as usize;
                }
                i += 32;
            }
            skip_ws_sse42(bytes, i)
        }
    }

    #[target_feature(enable = "avx2")]
    pub unsafe fn find_newline_avx2(bytes: &[u8], from: usize) -> usize {
        unsafe {
            let nl = _mm256_set1_epi8(b'\n' as i8);
            let mut i = from;
            while i + 32 <= bytes.len() {
                let v = _mm256_loadu_si256(bytes.as_ptr().add(i) as *const __m256i);
                let mask = _mm256_movemask_epi8(_mm256_cmpeq_epi8(v, nl)) as u32;
                if mask != 0 {
                    return i + mask.trailing_zeros() as usize;
                }
                i += 32;
            }
            find_newline_sse42(bytes, i)
        }
    }

    #[target_feature(enable = "avx2")]
    pub unsafe fn find_structural_avx2(bytes: &[u8], from: usize) -> usize {
        unsafe {
            let c_lbrace = _mm256_set1_epi8(b'{' as i8);
            let c_rbrace = _mm256_set1_epi8(b'}' as i8);
            let c_lbracket = _mm256_set1_epi8(b'[' as i8);
            let c_rbracket = _mm256_set1_epi8(b']' as i8);
            let c_comma = _mm256_set1_epi8(b',' as i8);
            let c_colon = _mm256_set1_epi8(b':' as i8);
            let c_quote = _mm256_set1_epi8(b'"' as i8);
            let c_slash = _mm256_set1_epi8(b'/' as i8);
            let mut i = from;
            while i + 32 <= bytes.len() {
                let v = _mm256_loadu_si256(bytes.as_ptr().add(i) as *const __m256i);
                let g1 = _mm256_or_si256(
                    _mm256_or_si256(
                        _mm256_cmpeq_epi8(v, c_lbrace),
                        _mm256_cmpeq_epi8(v, c_rbrace),
                    ),
                    _mm256_or_si256(
                        _mm256_cmpeq_epi8(v, c_lbracket),
                        _mm256_cmpeq_epi8(v, c_rbracket),
                    ),
                );
                let g2 = _mm256_or_si256(
                    _mm256_or_si256(_mm256_cmpeq_epi8(v, c_comma), _mm256_cmpeq_epi8(v, c_colon)),
                    _mm256_or_si256(_mm256_cmpeq_epi8(v, c_quote), _mm256_cmpeq_epi8(v, c_slash)),
                );
                let m = _mm256_or_si256(g1, g2);
                let mask = _mm256_movemask_epi8(m) as u32;
                if mask != 0 {
                    return i + mask.trailing_zeros() as usize;
                }
                i += 32;
            }
            find_structural_sse42(bytes, i)
        }
    }
}

// ---------------------------------------------------------------------------
// Dispatch (cached function-pointer table).
// ---------------------------------------------------------------------------

mod dispatch {
    use std::sync::atomic::{AtomicUsize, Ordering};

    type ScanFn = fn(&[u8], usize) -> usize;

    const UNSET: usize = 0;

    // Three slots — one per operation. Loaded lazily with Acquire; winning
    // writer stores Release. Ordering is correctness-safe for a plain
    // function pointer (no payload other than the pointer value).
    static FIND_STRING_END_FN: AtomicUsize = AtomicUsize::new(UNSET);
    static SKIP_WS_FN: AtomicUsize = AtomicUsize::new(UNSET);
    static FIND_NEWLINE_FN: AtomicUsize = AtomicUsize::new(UNSET);
    static FIND_STRUCTURAL_FN: AtomicUsize = AtomicUsize::new(UNSET);

    type FnTable = (ScanFn, ScanFn, ScanFn, ScanFn);

    #[cfg(all(feature = "simd", target_arch = "x86_64"))]
    fn pick() -> FnTable {
        fn se_avx2(b: &[u8], f: usize) -> usize {
            unsafe { super::x86::find_string_end_avx2(b, f) }
        }
        fn ws_avx2(b: &[u8], f: usize) -> usize {
            unsafe { super::x86::skip_ws_avx2(b, f) }
        }
        fn nl_avx2(b: &[u8], f: usize) -> usize {
            unsafe { super::x86::find_newline_avx2(b, f) }
        }
        fn st_avx2(b: &[u8], f: usize) -> usize {
            unsafe { super::x86::find_structural_avx2(b, f) }
        }
        fn se_sse42(b: &[u8], f: usize) -> usize {
            unsafe { super::x86::find_string_end_sse42(b, f) }
        }
        fn ws_sse42(b: &[u8], f: usize) -> usize {
            unsafe { super::x86::skip_ws_sse42(b, f) }
        }
        fn nl_sse42(b: &[u8], f: usize) -> usize {
            unsafe { super::x86::find_newline_sse42(b, f) }
        }
        fn st_sse42(b: &[u8], f: usize) -> usize {
            unsafe { super::x86::find_structural_sse42(b, f) }
        }

        if std::is_x86_feature_detected!("avx2") {
            (se_avx2, ws_avx2, nl_avx2, st_avx2)
        } else if std::is_x86_feature_detected!("sse4.2") {
            (se_sse42, ws_sse42, nl_sse42, st_sse42)
        } else {
            (
                super::scalar::find_string_end,
                super::scalar::skip_ws,
                super::scalar::find_newline,
                super::scalar::find_structural,
            )
        }
    }

    #[cfg(not(all(feature = "simd", target_arch = "x86_64")))]
    fn pick() -> FnTable {
        (
            super::scalar::find_string_end,
            super::scalar::skip_ws,
            super::scalar::find_newline,
            super::scalar::find_structural,
        )
    }

    #[cold]
    #[inline(never)]
    fn install() -> FnTable {
        let t = pick();
        FIND_STRING_END_FN.store(t.0 as usize, Ordering::Release);
        SKIP_WS_FN.store(t.1 as usize, Ordering::Release);
        FIND_NEWLINE_FN.store(t.2 as usize, Ordering::Release);
        FIND_STRUCTURAL_FN.store(t.3 as usize, Ordering::Release);
        t
    }

    #[inline]
    fn load_or_init(slot: &AtomicUsize, fallback: ScanFn) -> ScanFn {
        let raw = slot.load(Ordering::Acquire);
        if raw == UNSET {
            let _ = install();
            let raw = slot.load(Ordering::Acquire);
            if raw == UNSET {
                return fallback;
            }
            unsafe { std::mem::transmute::<usize, ScanFn>(raw) }
        } else {
            unsafe { std::mem::transmute::<usize, ScanFn>(raw) }
        }
    }

    #[inline]
    pub fn find_string_end(bytes: &[u8], from: usize) -> usize {
        load_or_init(&FIND_STRING_END_FN, super::scalar::find_string_end)(bytes, from)
    }
    #[inline]
    pub fn skip_ws(bytes: &[u8], from: usize) -> usize {
        load_or_init(&SKIP_WS_FN, super::scalar::skip_ws)(bytes, from)
    }
    #[inline]
    pub fn find_newline(bytes: &[u8], from: usize) -> usize {
        load_or_init(&FIND_NEWLINE_FN, super::scalar::find_newline)(bytes, from)
    }
    #[inline]
    pub fn find_structural(bytes: &[u8], from: usize) -> usize {
        load_or_init(&FIND_STRUCTURAL_FN, super::scalar::find_structural)(bytes, from)
    }
}

// ---------------------------------------------------------------------------
// Tests — scalar vs SIMD must agree on every input.
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    fn random_bytes(seed: u64, len: usize) -> Vec<u8> {
        // Xorshift64*, deterministic, no dependency.
        let mut s = seed.wrapping_add(0x9E37_79B9_7F4A_7C15);
        (0..len)
            .map(|_| {
                s ^= s << 13;
                s ^= s >> 7;
                s ^= s << 17;
                s as u8
            })
            .collect()
    }

    #[test]
    fn find_string_end_matches_scalar() {
        for seed in 0..8 {
            let buf = random_bytes(seed, 1024);
            for from in [0, 1, 7, 15, 16, 31, 32, 100, 500, 1023] {
                let a = super::scalar::find_string_end(&buf, from);
                let b = super::find_string_end(&buf, from);
                assert_eq!(a, b, "seed={seed} from={from}");
            }
        }
    }

    #[test]
    fn skip_ws_matches_scalar() {
        let mut buf = vec![b' '; 200];
        buf.extend_from_slice(b"\t\t\t\nX after");
        for from in [0, 16, 32, 100, 199, 200, 201] {
            let a = super::scalar::skip_ws(&buf, from);
            let b = super::skip_ws(&buf, from);
            assert_eq!(a, b, "from={from}");
        }
    }

    #[test]
    fn find_newline_matches_scalar() {
        let mut buf = vec![b'.'; 500];
        buf[100] = b'\n';
        buf[250] = b'\n';
        for from in [0, 16, 32, 100, 101, 200, 250, 260, 499] {
            let a = super::scalar::find_newline(&buf, from);
            let b = super::find_newline(&buf, from);
            assert_eq!(a, b, "from={from}");
        }
    }

    #[test]
    fn find_structural_public_and_scalar_agree() {
        // Short-remainder bypass (<16 bytes) goes through the scalar path.
        let small = b"a,b[c]:\"/";
        assert_eq!(super::find_structural(small, 0), 1);
        assert_eq!(super::scalar::find_structural(small, 2), 3);

        // Long input exercises the SIMD path in the dispatched function.
        let mut long = vec![b'x'; 200];
        long[80] = b':';
        long[150] = b',';
        assert_eq!(super::find_structural(&long, 0), 80);
        assert_eq!(super::find_structural(&long, 100), 150);
        assert_eq!(super::find_structural(&long, 160), long.len());

        // Scalar + dispatched must agree on random bytes.
        for seed in 0..4 {
            let buf = random_bytes(seed, 300);
            for from in [0, 1, 15, 16, 31, 32, 100, 299] {
                let a = super::scalar::find_structural(&buf, from);
                let b = super::find_structural(&buf, from);
                assert_eq!(a, b, "seed={seed} from={from}");
            }
        }

        // Trivial: absent-in-tail returns buf.len().
        let none = [b'x'; 32];
        assert_eq!(super::scalar::find_structural(&none, 0), none.len());
    }

    #[cfg(all(feature = "simd", target_arch = "x86_64"))]
    #[test]
    fn sse42_and_avx2_loop_continues_past_empty_chunks() {
        // Force the SIMD loop to traverse multiple 16/32-byte chunks with no
        // match before eventually finding one — exercises the post-chunk
        // `i += 16/32` continuation path.
        if std::is_x86_feature_detected!("sse4.2") {
            let mut buf = vec![b'x'; 128];
            buf[100] = b':';
            unsafe {
                assert_eq!(super::x86::find_structural_sse42(&buf, 0), 100);
                assert_eq!(super::x86::find_string_end_sse42(&buf, 0), buf.len());
                let mut ws = vec![b' '; 96];
                ws.extend_from_slice(b"X more");
                assert_eq!(super::x86::skip_ws_sse42(&ws, 0), 96);
                let mut nlbuf = vec![b'x'; 128];
                nlbuf[120] = b'\n';
                assert_eq!(super::x86::find_newline_sse42(&nlbuf, 0), 120);
            }
        }
        if std::is_x86_feature_detected!("avx2") {
            let mut buf = vec![b'x'; 256];
            buf[200] = b':';
            unsafe {
                assert_eq!(super::x86::find_structural_avx2(&buf, 0), 200);
                assert_eq!(super::x86::find_string_end_avx2(&buf, 0), buf.len());
                let mut ws = vec![b' '; 200];
                ws.extend_from_slice(b"X");
                assert_eq!(super::x86::skip_ws_avx2(&ws, 0), 200);
                let mut nlbuf = vec![b'x'; 256];
                nlbuf[240] = b'\n';
                assert_eq!(super::x86::find_newline_avx2(&nlbuf, 0), 240);
            }
        }
    }

    #[cfg(all(feature = "simd", target_arch = "x86_64"))]
    #[test]
    fn sse42_and_avx2_lanes_agree_with_scalar_when_available() {
        // Directly invoke the SSE4.2 and AVX2 implementations if the CPU
        // supports them, so coverage doesn't depend on the cached dispatch
        // selecting one particular lane.
        let mut buf = vec![b'x'; 256];
        buf[10] = b'"';
        buf[50] = b'\\';
        buf[100] = b'\n';
        buf[150] = b' ';
        buf[200] = b':';

        if std::is_x86_feature_detected!("sse4.2") {
            unsafe {
                assert_eq!(super::x86::find_string_end_sse42(&buf, 0), 10);
                assert_eq!(super::x86::find_string_end_sse42(&buf, 20), 50);
                assert_eq!(super::x86::find_newline_sse42(&buf, 0), 100);
                assert_eq!(super::x86::find_structural_sse42(&buf, 0), 10);

                // skip_ws_sse42: all-whitespace block then non-ws.
                let mut ws = vec![b' '; 64];
                ws.push(b'X');
                assert_eq!(super::x86::skip_ws_sse42(&ws, 0), 64);
                // Empty-remainder inputs fall through to scalar tail.
                assert_eq!(super::x86::skip_ws_sse42(b"   ", 0), 3);
                assert_eq!(super::x86::find_newline_sse42(b"abc", 0), 3);
                assert_eq!(super::x86::find_string_end_sse42(b"abc", 0), 3);
                assert_eq!(super::x86::find_structural_sse42(b"abc", 0), 3);
            }
        }

        if std::is_x86_feature_detected!("avx2") {
            unsafe {
                assert_eq!(super::x86::find_string_end_avx2(&buf, 0), 10);
                assert_eq!(super::x86::find_newline_avx2(&buf, 0), 100);
                assert_eq!(super::x86::find_structural_avx2(&buf, 0), 10);

                let mut ws = vec![b' '; 128];
                ws.push(b'X');
                assert_eq!(super::x86::skip_ws_avx2(&ws, 0), 128);
                // Short-remainder tail path in each avx2 function.
                assert_eq!(super::x86::skip_ws_avx2(b"   ", 0), 3);
                assert_eq!(super::x86::find_newline_avx2(b"abc", 0), 3);
                assert_eq!(super::x86::find_string_end_avx2(b"abc", 0), 3);
                assert_eq!(super::x86::find_structural_avx2(b"abc", 0), 3);
            }
        }
    }
}
