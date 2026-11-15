use super::super::catalog;
use super::super::error::FormulaError;
use super::FormulaValue;
use crate::NumberValue;
use std::ops::Range;

pub fn eval_call(
    name: &str,
    nums: &[NumberValue],
    span: Range<usize>,
) -> Result<FormulaValue, FormulaError> {
    let doc = catalog::find_function(name)
        .ok_or_else(|| FormulaError::unknown_function(span.clone(), name))?;
    let eval = doc
        .eval
        .ok_or_else(|| FormulaError::unknown_function(span.clone(), name))?;
    eval(nums, span)
}

fn unary_float<F: Fn(f64) -> f64>(
    nums: &[NumberValue],
    f: F,
) -> Result<FormulaValue, FormulaError> {
    Ok(FormulaValue::Number(
        NumberValue::Float(f(nums[0].to_f64())).normalize(),
    ))
}

fn check_positive(
    nums: &[NumberValue],
    span: Range<usize>,
    name: &'static str,
) -> Result<f64, FormulaError> {
    let f = nums[0].to_f64();
    if f <= 0.0 {
        return Err(FormulaError::domain_error(span, name));
    }
    Ok(f)
}

pub fn eval_abs(nums: &[NumberValue], _span: Range<usize>) -> Result<FormulaValue, FormulaError> {
    let res = match nums[0] {
        NumberValue::Int(i) => i
            .checked_abs()
            .map(NumberValue::Int)
            .unwrap_or_else(|| NumberValue::Float(-(i as f64))),
        NumberValue::Float(f) => NumberValue::Float(f.abs()).normalize(),
    };
    Ok(FormulaValue::Number(res))
}

pub fn eval_sign(nums: &[NumberValue], _span: Range<usize>) -> Result<FormulaValue, FormulaError> {
    let res = match nums[0] {
        NumberValue::Int(i) => NumberValue::Int(i.signum()),
        NumberValue::Float(f) => {
            if f > 0.0 {
                NumberValue::Int(1)
            } else if f < 0.0 {
                NumberValue::Int(-1)
            } else {
                NumberValue::Int(0)
            }
        }
    };
    Ok(FormulaValue::Number(res))
}

pub fn eval_min(nums: &[NumberValue], _span: Range<usize>) -> Result<FormulaValue, FormulaError> {
    let all_int = nums.iter().all(|n| matches!(n, NumberValue::Int(_)));
    if all_int {
        let min = nums
            .iter()
            .map(|n| match n {
                NumberValue::Int(i) => *i,
                _ => unreachable!(),
            })
            .min()
            .unwrap_or(0);
        Ok(FormulaValue::Number(NumberValue::Int(min)))
    } else {
        let min = nums
            .iter()
            .map(|n| n.to_f64())
            .fold(f64::INFINITY, f64::min);
        Ok(FormulaValue::Number(NumberValue::Float(min).normalize()))
    }
}

pub fn eval_max(nums: &[NumberValue], _span: Range<usize>) -> Result<FormulaValue, FormulaError> {
    let all_int = nums.iter().all(|n| matches!(n, NumberValue::Int(_)));
    if all_int {
        let max = nums
            .iter()
            .map(|n| match n {
                NumberValue::Int(i) => *i,
                _ => unreachable!(),
            })
            .max()
            .unwrap_or(0);
        Ok(FormulaValue::Number(NumberValue::Int(max)))
    } else {
        let max = nums
            .iter()
            .map(|n| n.to_f64())
            .fold(f64::NEG_INFINITY, f64::max);
        Ok(FormulaValue::Number(NumberValue::Float(max).normalize()))
    }
}

pub fn eval_clamp(nums: &[NumberValue], span: Range<usize>) -> Result<FormulaValue, FormulaError> {
    if nums[1].to_f64() > nums[2].to_f64() {
        return Err(FormulaError::domain_error(span, "clamp"));
    }
    if let (NumberValue::Int(x), NumberValue::Int(lo), NumberValue::Int(hi)) =
        (nums[0], nums[1], nums[2])
    {
        Ok(FormulaValue::Number(NumberValue::Int(x.clamp(lo, hi))))
    } else {
        let f = nums[0].to_f64().clamp(nums[1].to_f64(), nums[2].to_f64());
        Ok(FormulaValue::Number(NumberValue::Float(f).normalize()))
    }
}

pub fn eval_round(nums: &[NumberValue], span: Range<usize>) -> Result<FormulaValue, FormulaError> {
    let digits = if nums.len() > 1 {
        let d = nums[1].to_f64();
        if d < 0.0 || d.fract() != 0.0 {
            return Err(FormulaError::domain_error(span, "round"));
        }
        d as i32
    } else {
        0
    };
    if digits == 0 {
        match nums[0] {
            NumberValue::Int(i) => Ok(FormulaValue::Number(NumberValue::Int(i))),
            NumberValue::Float(f) => Ok(FormulaValue::Number(
                NumberValue::Float(f.round()).normalize(),
            )),
        }
    } else {
        let factor = 10f64.powi(digits);
        let f = (nums[0].to_f64() * factor).round() / factor;
        Ok(FormulaValue::Number(NumberValue::Float(f).normalize()))
    }
}

pub fn eval_floor(nums: &[NumberValue], _span: Range<usize>) -> Result<FormulaValue, FormulaError> {
    match nums[0] {
        NumberValue::Int(i) => Ok(FormulaValue::Number(NumberValue::Int(i))),
        NumberValue::Float(f) => Ok(FormulaValue::Number(
            NumberValue::Float(f.floor()).normalize(),
        )),
    }
}

pub fn eval_ceil(nums: &[NumberValue], _span: Range<usize>) -> Result<FormulaValue, FormulaError> {
    match nums[0] {
        NumberValue::Int(i) => Ok(FormulaValue::Number(NumberValue::Int(i))),
        NumberValue::Float(f) => Ok(FormulaValue::Number(
            NumberValue::Float(f.ceil()).normalize(),
        )),
    }
}

pub fn eval_trunc(nums: &[NumberValue], _span: Range<usize>) -> Result<FormulaValue, FormulaError> {
    match nums[0] {
        NumberValue::Int(i) => Ok(FormulaValue::Number(NumberValue::Int(i))),
        NumberValue::Float(f) => Ok(FormulaValue::Number(
            NumberValue::Float(f.trunc()).normalize(),
        )),
    }
}

pub fn eval_frac(nums: &[NumberValue], _span: Range<usize>) -> Result<FormulaValue, FormulaError> {
    match nums[0] {
        NumberValue::Int(_) => Ok(FormulaValue::Number(NumberValue::Int(0))),
        NumberValue::Float(f) => Ok(FormulaValue::Number(
            NumberValue::Float(f.fract()).normalize(),
        )),
    }
}

pub fn eval_sqrt(nums: &[NumberValue], span: Range<usize>) -> Result<FormulaValue, FormulaError> {
    let f = nums[0].to_f64();
    if f < 0.0 {
        return Err(FormulaError::domain_error(span, "sqrt"));
    }
    if let NumberValue::Int(i) = nums[0] {
        let s = (i as f64).sqrt().round() as i64;
        if s >= 0 && s * s == i {
            return Ok(FormulaValue::Number(NumberValue::Int(s)));
        }
    }
    Ok(FormulaValue::Number(
        NumberValue::Float(f.sqrt()).normalize(),
    ))
}

pub fn eval_cbrt(nums: &[NumberValue], _span: Range<usize>) -> Result<FormulaValue, FormulaError> {
    if let NumberValue::Int(i) = nums[0] {
        let c = (i as f64).cbrt().round() as i64;
        if c * c * c == i {
            return Ok(FormulaValue::Number(NumberValue::Int(c)));
        }
    }
    let f = nums[0].to_f64().cbrt();
    Ok(FormulaValue::Number(NumberValue::Float(f).normalize()))
}

pub fn eval_pow(nums: &[NumberValue], span: Range<usize>) -> Result<FormulaValue, FormulaError> {
    super::eval_pow(nums[0], nums[1], span)
}

pub fn eval_exp(nums: &[NumberValue], span: Range<usize>) -> Result<FormulaValue, FormulaError> {
    let f = nums[0].to_f64().exp();
    super::check_float(f, span).map(|n| FormulaValue::Number(n.normalize()))
}

pub fn eval_ln(nums: &[NumberValue], span: Range<usize>) -> Result<FormulaValue, FormulaError> {
    let f = check_positive(nums, span, "ln")?;
    Ok(FormulaValue::Number(NumberValue::Float(f.ln()).normalize()))
}

pub fn eval_log(nums: &[NumberValue], span: Range<usize>) -> Result<FormulaValue, FormulaError> {
    let f = check_positive(nums, span.clone(), "log")?;
    let base = if nums.len() > 1 {
        nums[1].to_f64()
    } else {
        std::f64::consts::E
    };
    if base <= 0.0 || (base - 1.0).abs() < 1e-12 {
        return Err(FormulaError::domain_error(span, "log"));
    }
    let res = f.ln() / base.ln();
    super::check_float(res, span).map(|n| FormulaValue::Number(n.normalize()))
}

pub fn eval_log10(nums: &[NumberValue], span: Range<usize>) -> Result<FormulaValue, FormulaError> {
    let f = check_positive(nums, span, "log10")?;
    Ok(FormulaValue::Number(
        NumberValue::Float(f.log10()).normalize(),
    ))
}

pub fn eval_log2(nums: &[NumberValue], span: Range<usize>) -> Result<FormulaValue, FormulaError> {
    let f = check_positive(nums, span, "log2")?;
    Ok(FormulaValue::Number(
        NumberValue::Float(f.log2()).normalize(),
    ))
}

pub fn eval_hypot(nums: &[NumberValue], _span: Range<usize>) -> Result<FormulaValue, FormulaError> {
    let sum_sq: f64 = nums.iter().map(|n| n.to_f64().powi(2)).sum();
    Ok(FormulaValue::Number(
        NumberValue::Float(sum_sq.sqrt()).normalize(),
    ))
}

pub fn eval_sin(nums: &[NumberValue], _span: Range<usize>) -> Result<FormulaValue, FormulaError> {
    unary_float(nums, f64::sin)
}

pub fn eval_cos(nums: &[NumberValue], _span: Range<usize>) -> Result<FormulaValue, FormulaError> {
    unary_float(nums, f64::cos)
}

pub fn eval_tan(nums: &[NumberValue], _span: Range<usize>) -> Result<FormulaValue, FormulaError> {
    unary_float(nums, f64::tan)
}

pub fn eval_asin(nums: &[NumberValue], span: Range<usize>) -> Result<FormulaValue, FormulaError> {
    let f = nums[0].to_f64();
    if !(-1.0..=1.0).contains(&f) {
        return Err(FormulaError::domain_error(span, "asin"));
    }
    unary_float(nums, f64::asin)
}

pub fn eval_acos(nums: &[NumberValue], span: Range<usize>) -> Result<FormulaValue, FormulaError> {
    let f = nums[0].to_f64();
    if !(-1.0..=1.0).contains(&f) {
        return Err(FormulaError::domain_error(span, "acos"));
    }
    unary_float(nums, f64::acos)
}

pub fn eval_atan(nums: &[NumberValue], _span: Range<usize>) -> Result<FormulaValue, FormulaError> {
    unary_float(nums, f64::atan)
}

pub fn eval_atan2(nums: &[NumberValue], _span: Range<usize>) -> Result<FormulaValue, FormulaError> {
    Ok(FormulaValue::Number(
        NumberValue::Float(nums[0].to_f64().atan2(nums[1].to_f64())).normalize(),
    ))
}

pub fn eval_sinh(nums: &[NumberValue], _span: Range<usize>) -> Result<FormulaValue, FormulaError> {
    unary_float(nums, f64::sinh)
}

pub fn eval_cosh(nums: &[NumberValue], _span: Range<usize>) -> Result<FormulaValue, FormulaError> {
    unary_float(nums, f64::cosh)
}

pub fn eval_tanh(nums: &[NumberValue], _span: Range<usize>) -> Result<FormulaValue, FormulaError> {
    unary_float(nums, f64::tanh)
}

pub fn eval_deg(nums: &[NumberValue], _span: Range<usize>) -> Result<FormulaValue, FormulaError> {
    unary_float(nums, |x| x * 180.0 / std::f64::consts::PI)
}

pub fn eval_rad(nums: &[NumberValue], _span: Range<usize>) -> Result<FormulaValue, FormulaError> {
    unary_float(nums, |x| x * std::f64::consts::PI / 180.0)
}

pub fn eval_sum(nums: &[NumberValue], span: Range<usize>) -> Result<FormulaValue, FormulaError> {
    let all_int = nums.iter().all(|n| matches!(n, NumberValue::Int(_)));
    if all_int {
        let mut acc: i64 = 0;
        let mut overflowed = false;
        for n in nums {
            if let NumberValue::Int(i) = n {
                if let Some(sum) = acc.checked_add(*i) {
                    acc = sum;
                } else {
                    overflowed = true;
                    break;
                }
            }
        }
        if !overflowed {
            return Ok(FormulaValue::Number(NumberValue::Int(acc)));
        }
    }
    let sum: f64 = nums.iter().map(|n| n.to_f64()).sum();
    super::check_float(sum, span).map(FormulaValue::Number)
}

pub fn eval_avg(nums: &[NumberValue], span: Range<usize>) -> Result<FormulaValue, FormulaError> {
    let sum: f64 = nums.iter().map(|n| n.to_f64()).sum();
    let avg = sum / (nums.len() as f64);
    super::check_float(avg, span).map(|n| FormulaValue::Number(n.normalize()))
}

pub fn eval_median(nums: &[NumberValue], span: Range<usize>) -> Result<FormulaValue, FormulaError> {
    let mut floats: Vec<f64> = nums.iter().map(|n| n.to_f64()).collect();
    floats.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let len = floats.len();
    let med = if len % 2 == 1 {
        floats[len / 2]
    } else {
        (floats[len / 2 - 1] + floats[len / 2]) / 2.0
    };
    super::check_float(med, span).map(|n| FormulaValue::Number(n.normalize()))
}

pub fn eval_lerp(nums: &[NumberValue], span: Range<usize>) -> Result<FormulaValue, FormulaError> {
    let a = nums[0].to_f64();
    let b = nums[1].to_f64();
    let t = nums[2].to_f64();
    let res = a + (b - a) * t;
    super::check_float(res, span).map(|n| FormulaValue::Number(n.normalize()))
}

pub fn eval_map(nums: &[NumberValue], span: Range<usize>) -> Result<FormulaValue, FormulaError> {
    let x = nums[0].to_f64();
    let in_lo = nums[1].to_f64();
    let in_hi = nums[2].to_f64();
    let out_lo = nums[3].to_f64();
    let out_hi = nums[4].to_f64();
    let in_range = in_hi - in_lo;
    if in_range.abs() < 1e-15 {
        return Err(FormulaError::division_by_zero(span));
    }
    let factor = (x - in_lo) / in_range;
    let out = out_lo + factor * (out_hi - out_lo);
    super::check_float(out, span).map(|n| FormulaValue::Number(n.normalize()))
}

fn mod_int(x: i64, y: i64) -> i64 {
    let rem = x % y;
    if (rem > 0 && y < 0) || (rem < 0 && y > 0) {
        rem + y
    } else {
        rem
    }
}

fn mod_float(a: f64, b: f64) -> f64 {
    let rem = a % b;
    if (rem > 0.0 && b < 0.0) || (rem < 0.0 && b > 0.0) {
        rem + b
    } else {
        rem
    }
}

pub fn eval_mod(nums: &[NumberValue], span: Range<usize>) -> Result<FormulaValue, FormulaError> {
    let b = nums[1].to_f64();
    if b == 0.0 {
        return Err(FormulaError::division_by_zero(span));
    }
    match (nums[0], nums[1]) {
        (NumberValue::Int(x), NumberValue::Int(y)) => {
            Ok(FormulaValue::Number(NumberValue::Int(mod_int(x, y))))
        }
        _ => {
            let res = mod_float(nums[0].to_f64(), b);
            super::check_float(res, span).map(|n| FormulaValue::Number(n.normalize()))
        }
    }
}

pub fn eval_gcd(nums: &[NumberValue], span: Range<usize>) -> Result<FormulaValue, FormulaError> {
    let mut acc = int_arg(&nums[0], span.clone(), "gcd")?.abs();
    for n in &nums[1..] {
        let val = int_arg(n, span.clone(), "gcd")?.abs();
        acc = gcd_pair(acc, val);
    }
    Ok(FormulaValue::Number(NumberValue::Int(acc)))
}

pub fn eval_lcm(nums: &[NumberValue], span: Range<usize>) -> Result<FormulaValue, FormulaError> {
    let mut acc = int_arg(&nums[0], span.clone(), "lcm")?.abs();
    for n in &nums[1..] {
        let val = int_arg(n, span.clone(), "lcm")?.abs();
        if acc == 0 || val == 0 {
            acc = 0;
        } else {
            let g = gcd_pair(acc, val);
            if let Some(m) = (acc / g).checked_mul(val) {
                acc = m;
            } else {
                let f = (acc as f64 / g as f64) * val as f64;
                return super::check_float(f, span).map(|n| FormulaValue::Number(n.normalize()));
            }
        }
    }
    Ok(FormulaValue::Number(NumberValue::Int(acc)))
}

pub fn eval_between(
    nums: &[NumberValue],
    _span: Range<usize>,
) -> Result<FormulaValue, FormulaError> {
    let x = nums[0].to_f64();
    let lo = nums[1].to_f64();
    let hi = nums[2].to_f64();
    Ok(FormulaValue::Bool(lo <= x && x <= hi))
}

pub fn eval_isnan(nums: &[NumberValue], _span: Range<usize>) -> Result<FormulaValue, FormulaError> {
    Ok(FormulaValue::Bool(nums[0].to_f64().is_nan()))
}

pub fn eval_isfinite(
    nums: &[NumberValue],
    _span: Range<usize>,
) -> Result<FormulaValue, FormulaError> {
    Ok(FormulaValue::Bool(nums[0].to_f64().is_finite()))
}

fn int_arg(n: &NumberValue, span: Range<usize>, fn_name: &str) -> Result<i64, FormulaError> {
    match n {
        NumberValue::Int(i) => Ok(*i),
        NumberValue::Float(f) => {
            if f.fract() == 0.0 && *f <= i64::MAX as f64 && *f >= i64::MIN as f64 {
                Ok(*f as i64)
            } else {
                Err(FormulaError::domain_error(span, fn_name))
            }
        }
    }
}

fn gcd_pair(mut a: i64, mut b: i64) -> i64 {
    while b != 0 {
        let t = b;
        b = a % b;
        a = t;
    }
    a.abs()
}
