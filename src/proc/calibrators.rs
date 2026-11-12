//! Evaluation of calibrators (raw -> engineering value conversion).
//! The algorithms follow the Yamcs implementation.

use crate::mdb::types::{Calibrator, MathOpElement, MathOperator, SplineCalibrator};

use super::{ProcError, Result};

/// applies the calibrator to the raw value
pub fn calibrate(cal: &Calibrator, x: f64) -> Result<f64> {
    match cal {
        Calibrator::Polynomial(coefficients) => Ok(polynomial(coefficients, x)),
        Calibrator::Spline(spline) => Ok(spline_calibrate(spline, x)),
        Calibrator::MathOperation(elements) => math_operation(elements, x, &[]),
        Calibrator::Unsupported(name) => {
            Err(ProcError::DecodingError(format!("unsupported calibrator {}", name)))
        }
    }
}

/// evaluates the polynomial using Horner's method; coefficients[i] corresponds to x^i
fn polynomial(coefficients: &[f64], x: f64) -> f64 {
    coefficients.iter().rev().fold(0.0, |acc, c| acc * x + c)
}

/// linear interpolation between the points, linear extrapolation outside them (like Yamcs)
fn spline_calibrate(spline: &SplineCalibrator, x: f64) -> f64 {
    let points = &spline.points;
    let n = points.len();
    // the parser ensures that there are at least two points
    debug_assert!(n >= 2);

    let mut i = points.iter().position(|p| p.raw >= x).unwrap_or(n);
    let j;
    if i == 0 {
        j = 0;
        i = 1;
    } else if i == n {
        j = n - 2;
        i = n - 1;
    } else {
        j = i - 1;
    }
    let (a1, b1) = (points[i].raw, points[i].calibrated);
    let (a2, b2) = (points[j].raw, points[j].calibrated);

    ((b1 - b2) * x + (a1 * b2 - b1 * a2)) / (a1 - a2)
}

/// evaluates the math operation expressed in reverse polish notation.
/// x is the value of ThisParameter and inputs the values of the algorithm inputs
pub fn math_operation(elements: &[MathOpElement], x: f64, inputs: &[f64]) -> Result<f64> {
    let mut stack: Vec<f64> = Vec::with_capacity(8);
    let underflow = || ProcError::InvalidMdb("invalid math operation: stack underflow".to_owned());

    for el in elements {
        match el {
            MathOpElement::Value(v) => stack.push(*v),
            MathOpElement::ThisParameter => stack.push(x),
            MathOpElement::Input(idx) => stack.push(*inputs.get(*idx).ok_or_else(|| {
                ProcError::InvalidMdb(format!("invalid math operation: no input {}", idx))
            })?),
            MathOpElement::Operator(op) => {
                use MathOperator::*;
                match op {
                    Swap => {
                        let r = stack.pop().ok_or_else(underflow)?;
                        let l = stack.pop().ok_or_else(underflow)?;
                        stack.push(r);
                        stack.push(l);
                    }
                    Drop => {
                        stack.pop().ok_or_else(underflow)?;
                    }
                    Dup => {
                        let l = *stack.last().ok_or_else(underflow)?;
                        stack.push(l);
                    }
                    Over => {
                        if stack.len() < 2 {
                            return Err(underflow());
                        }
                        stack.push(stack[stack.len() - 2]);
                    }
                    _ => {
                        let (consumed, _) = op.stack_effect();
                        let v = if consumed == 2 {
                            let r = stack.pop().ok_or_else(underflow)?;
                            let l = stack.pop().ok_or_else(underflow)?;
                            binary_op(*op, l, r)
                        } else {
                            let l = stack.pop().ok_or_else(underflow)?;
                            unary_op(*op, l)
                        };
                        stack.push(v);
                    }
                }
            }
        }
    }
    if stack.len() != 1 {
        return Err(ProcError::InvalidMdb(format!(
            "invalid math operation: {} elements on the stack at the end of the evaluation",
            stack.len()
        )));
    }
    Ok(stack[0])
}

fn binary_op(op: MathOperator, l: f64, r: f64) -> f64 {
    use MathOperator::*;
    match op {
        Plus => l + r,
        Minus => l - r,
        Star => l * r,
        Div => l / r,
        // like the Java remainder, the sign follows the dividend
        Modulo => l % r,
        Pow => l.powf(r),
        RevPow => r.powf(l),
        LeftShift => ((l as i64).wrapping_shl(r as i64 as u32)) as f64,
        RightShift => ((l as i64).wrapping_shr(r as i64 as u32)) as f64,
        BitwiseOr => ((l as i64) | (r as i64)) as f64,
        BitwiseAnd => ((l as i64) & (r as i64)) as f64,
        _ => unreachable!("not a binary operator: {:?}", op),
    }
}

fn unary_op(op: MathOperator, l: f64) -> f64 {
    use MathOperator::*;
    match op {
        Ln => l.ln(),
        Log => l.log10(),
        Exp => l.exp(),
        Inv => 1.0 / l,
        Fact => factorial(l),
        Tan => l.tan(),
        Cos => l.cos(),
        Sin => l.sin(),
        Atan => l.atan(),
        Acos => l.acos(),
        Asin => l.asin(),
        Tanh => l.tanh(),
        Cosh => l.cosh(),
        Sinh => l.sinh(),
        Atanh => l.atanh(),
        Acosh => l.acosh(),
        Asinh => l.asinh(),
        Abs => l.abs(),
        _ => unreachable!("not an unary operator: {:?}", op),
    }
}

/// factorial of the integer part of x; NaN for negative values
fn factorial(x: f64) -> f64 {
    if x < 0.0 || x.is_nan() {
        return f64::NAN;
    }
    let n = x as u64;
    if n > 170 {
        return f64::INFINITY;
    }
    (1..=n).fold(1.0, |acc, k| acc * k as f64)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mdb::types::SplinePoint;

    fn spline(points: &[(f64, f64)]) -> Calibrator {
        Calibrator::Spline(SplineCalibrator {
            order: 1,
            extrapolate: false,
            points: points.iter().map(|&(raw, calibrated)| SplinePoint { raw, calibrated }).collect(),
        })
    }

    #[test]
    fn test_polynomial() {
        let cal = Calibrator::Polynomial(vec![-10.0, 5.0]);
        assert_eq!(-10.0, calibrate(&cal, 0.0).unwrap());
        assert_eq!(10.0, calibrate(&cal, 4.0).unwrap());
        let cal = Calibrator::Polynomial(vec![1.0, 0.0, 2.0]);
        assert_eq!(19.0, calibrate(&cal, 3.0).unwrap());
        assert_eq!(0.0, calibrate(&Calibrator::Polynomial(vec![]), 3.0).unwrap());
    }

    #[test]
    fn test_spline() {
        let cal = spline(&[(-6.0, 1.0), (-3.0, 2.0), (-1.0, 3.0), (1.0, 3.0), (3.0, 2.0), (6.0, 1.0)]);
        assert_eq!(1.0, calibrate(&cal, -6.0).unwrap());
        assert_eq!(2.0, calibrate(&cal, -3.0).unwrap());
        assert_eq!(2.5, calibrate(&cal, -2.0).unwrap());
        assert_eq!(3.0, calibrate(&cal, 0.0).unwrap());
        assert_eq!(1.0, calibrate(&cal, 6.0).unwrap());
        // extrapolation
        assert_eq!(0.0, calibrate(&cal, -9.0).unwrap());
        assert_eq!(0.0, calibrate(&cal, 9.0).unwrap());
    }

    #[test]
    fn test_math_operation() {
        use MathOpElement::*;
        use MathOperator::*;
        // 0.5 * x + 1.25
        let cal = Calibrator::MathOperation(vec![
            Value(0.5),
            ThisParameter,
            Operator(Star),
            Value(1.25),
            Operator(Plus),
        ]);
        assert_eq!(6.25, calibrate(&cal, 10.0).unwrap());

        // 64 * ln(1.234 * x) / ln(2)
        let cal = Calibrator::MathOperation(vec![
            Value(1.234),
            ThisParameter,
            Operator(Star),
            Operator(Ln),
            Value(2.0),
            Operator(Ln),
            Operator(Div),
            Value(64.0),
            Operator(Star),
        ]);
        let expected = 64.0 * (1.234f64 * 5.0).ln() / 2f64.ln();
        assert!((expected - calibrate(&cal, 5.0).unwrap()).abs() < 1e-12);

        // stack operations: x x * (square), 2 x swap - => x - 2
        let cal = Calibrator::MathOperation(vec![ThisParameter, Operator(Dup), Operator(Star)]);
        assert_eq!(9.0, calibrate(&cal, 3.0).unwrap());
        let cal = Calibrator::MathOperation(vec![Value(2.0), ThisParameter, Operator(Swap), Operator(Minus)]);
        assert_eq!(1.0, calibrate(&cal, 3.0).unwrap());
        let cal = Calibrator::MathOperation(vec![ThisParameter, Value(4.0), Operator(LeftShift)]);
        assert_eq!(48.0, calibrate(&cal, 3.0).unwrap());
        let cal = Calibrator::MathOperation(vec![ThisParameter, Operator(Fact)]);
        assert_eq!(120.0, calibrate(&cal, 5.0).unwrap());
    }
}
