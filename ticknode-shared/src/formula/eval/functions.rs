use super::super::error::FormulaError;
use crate::NumberValue;
use std::ops::Range;

pub fn eval_call(
    name: &str,
    nums: &[NumberValue],
    span: Range<usize>,
) -> Result<super::FormulaValue, FormulaError> {
    use super::FormulaValue;

    match name {
        "abs" => {
            let res = match nums[0] {
                NumberValue::Int(i) => i
                    .checked_abs()
                    .map(NumberValue::Int)
                    .unwrap_or_else(|| NumberValue::Float(-(i as f64))),
                NumberValue::Float(f) => NumberValue::Float(f.abs()).normalize(),
            };
            Ok(FormulaValue::Number(res))
        }
        "sign" => {
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
        "min" => {
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
        "max" => {
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
        "clamp" => {
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
        "round" => {
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
        "floor" => match nums[0] {
            NumberValue::Int(i) => Ok(FormulaValue::Number(NumberValue::Int(i))),
            NumberValue::Float(f) => Ok(FormulaValue::Number(
                NumberValue::Float(f.floor()).normalize(),
            )),
        },
        "ceil" => match nums[0] {
            NumberValue::Int(i) => Ok(FormulaValue::Number(NumberValue::Int(i))),
            NumberValue::Float(f) => Ok(FormulaValue::Number(
                NumberValue::Float(f.ceil()).normalize(),
            )),
        },
        "trunc" => match nums[0] {
            NumberValue::Int(i) => Ok(FormulaValue::Number(NumberValue::Int(i))),
            NumberValue::Float(f) => Ok(FormulaValue::Number(
                NumberValue::Float(f.trunc()).normalize(),
            )),
        },
        "frac" => match nums[0] {
            NumberValue::Int(_) => Ok(FormulaValue::Number(NumberValue::Int(0))),
            NumberValue::Float(f) => Ok(FormulaValue::Number(
                NumberValue::Float(f.fract()).normalize(),
            )),
        },
        "sqrt" => {
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
        "cbrt" => {
            if let NumberValue::Int(i) = nums[0] {
                let c = (i as f64).cbrt().round() as i64;
                if c * c * c == i {
                    return Ok(FormulaValue::Number(NumberValue::Int(c)));
                }
            }
            let f = nums[0].to_f64().cbrt();
            Ok(FormulaValue::Number(NumberValue::Float(f).normalize()))
        }
        "pow" => super::eval_pow(nums[0], nums[1], span),
        "exp" => {
            let f = nums[0].to_f64().exp();
            super::check_float(f, span).map(|n| FormulaValue::Number(n.normalize()))
        }
        "ln" => {
            let f = nums[0].to_f64();
            if f <= 0.0 {
                return Err(FormulaError::domain_error(span, "ln"));
            }
            Ok(FormulaValue::Number(NumberValue::Float(f.ln()).normalize()))
        }
        "log" => {
            let f = nums[0].to_f64();
            if f <= 0.0 {
                return Err(FormulaError::domain_error(span, "log"));
            }
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
        "log10" => {
            let f = nums[0].to_f64();
            if f <= 0.0 {
                return Err(FormulaError::domain_error(span, "log10"));
            }
            Ok(FormulaValue::Number(
                NumberValue::Float(f.log10()).normalize(),
            ))
        }
        "log2" => {
            let f = nums[0].to_f64();
            if f <= 0.0 {
                return Err(FormulaError::domain_error(span, "log2"));
            }
            Ok(FormulaValue::Number(
                NumberValue::Float(f.log2()).normalize(),
            ))
        }
        "hypot" => {
            let sum_sq: f64 = nums.iter().map(|n| n.to_f64().powi(2)).sum();
            Ok(FormulaValue::Number(
                NumberValue::Float(sum_sq.sqrt()).normalize(),
            ))
        }
        "sin" => Ok(FormulaValue::Number(
            NumberValue::Float(nums[0].to_f64().sin()).normalize(),
        )),
        "cos" => Ok(FormulaValue::Number(
            NumberValue::Float(nums[0].to_f64().cos()).normalize(),
        )),
        "tan" => Ok(FormulaValue::Number(
            NumberValue::Float(nums[0].to_f64().tan()).normalize(),
        )),
        "asin" => {
            let f = nums[0].to_f64();
            if !(-1.0..=1.0).contains(&f) {
                return Err(FormulaError::domain_error(span, "asin"));
            }
            Ok(FormulaValue::Number(
                NumberValue::Float(f.asin()).normalize(),
            ))
        }
        "acos" => {
            let f = nums[0].to_f64();
            if !(-1.0..=1.0).contains(&f) {
                return Err(FormulaError::domain_error(span, "acos"));
            }
            Ok(FormulaValue::Number(
                NumberValue::Float(f.acos()).normalize(),
            ))
        }
        "atan" => Ok(FormulaValue::Number(
            NumberValue::Float(nums[0].to_f64().atan()).normalize(),
        )),
        "atan2" => Ok(FormulaValue::Number(
            NumberValue::Float(nums[0].to_f64().atan2(nums[1].to_f64())).normalize(),
        )),
        "sinh" => Ok(FormulaValue::Number(
            NumberValue::Float(nums[0].to_f64().sinh()).normalize(),
        )),
        "cosh" => Ok(FormulaValue::Number(
            NumberValue::Float(nums[0].to_f64().cosh()).normalize(),
        )),
        "tanh" => Ok(FormulaValue::Number(
            NumberValue::Float(nums[0].to_f64().tanh()).normalize(),
        )),
        "deg" => {
            let f = nums[0].to_f64() * 180.0 / std::f64::consts::PI;
            Ok(FormulaValue::Number(NumberValue::Float(f).normalize()))
        }
        "rad" => {
            let f = nums[0].to_f64() * std::f64::consts::PI / 180.0;
            Ok(FormulaValue::Number(NumberValue::Float(f).normalize()))
        }
        "sum" => {
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
        "avg" => {
            let sum: f64 = nums.iter().map(|n| n.to_f64()).sum();
            let avg = sum / (nums.len() as f64);
            super::check_float(avg, span).map(|n| FormulaValue::Number(n.normalize()))
        }
        "median" => {
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
        "lerp" => {
            let a = nums[0].to_f64();
            let b = nums[1].to_f64();
            let t = nums[2].to_f64();
            let res = a + (b - a) * t;
            super::check_float(res, span).map(|n| FormulaValue::Number(n.normalize()))
        }
        "map" => {
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
        "mod" => {
            let b = nums[1].to_f64();
            if b == 0.0 {
                return Err(FormulaError::division_by_zero(span));
            }
            if let (NumberValue::Int(x), NumberValue::Int(y)) = (nums[0], nums[1]) {
                let rem = x % y;
                let res = if (rem > 0 && y < 0) || (rem < 0 && y > 0) {
                    rem + y
                } else {
                    rem
                };
                Ok(FormulaValue::Number(NumberValue::Int(res)))
            } else {
                let a = nums[0].to_f64();
                let rem = a % b;
                let res = if (rem > 0.0 && b < 0.0) || (rem < 0.0 && b > 0.0) {
                    rem + b
                } else {
                    rem
                };
                super::check_float(res, span).map(|n| FormulaValue::Number(n.normalize()))
            }
        }
        "gcd" => {
            let mut acc = int_arg(&nums[0], span.clone(), "gcd")?.abs();
            for n in &nums[1..] {
                let val = int_arg(n, span.clone(), "gcd")?.abs();
                acc = gcd_pair(acc, val);
            }
            Ok(FormulaValue::Number(NumberValue::Int(acc)))
        }
        "lcm" => {
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
                        return super::check_float(f, span)
                            .map(|n| FormulaValue::Number(n.normalize()));
                    }
                }
            }
            Ok(FormulaValue::Number(NumberValue::Int(acc)))
        }
        "between" => {
            let x = nums[0].to_f64();
            let lo = nums[1].to_f64();
            let hi = nums[2].to_f64();
            Ok(FormulaValue::Bool(lo <= x && x <= hi))
        }
        "isnan" => Ok(FormulaValue::Bool(nums[0].to_f64().is_nan())),
        "isfinite" => Ok(FormulaValue::Bool(nums[0].to_f64().is_finite())),
        _ => Err(FormulaError::unknown_function(span, name)),
    }
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
