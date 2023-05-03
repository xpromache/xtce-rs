use std::mem::discriminant;

use crate::{
    mdb::{
        debug::MdbItemDebug, utils::get_member_type, Comparison, ComparisonOperator,
        MissionDatabase, NamedItem, ParameterInstanceRef,
    },
    value::Value, proc::ProcError
};

use super::{ProcCtx, Result};

pub(crate) trait CriteriaEvaluator {
    fn evaluate(&self, ctx: &ProcCtx) -> MatchResult;
}

struct OrEvaluator {
    list: Vec<Box<dyn CriteriaEvaluator>>,
}

struct AndEvaluator {
    list: Vec<Box<dyn CriteriaEvaluator>>,
}

#[derive(PartialEq, Debug)]
pub enum MatchResult {
    /// condition matches
    OK,
    /// condition does not match
    NOK,
    /// matching cannot be determined because not all inputs are available
    UNDEF,
    /// There was an error performing the match - for example comparing things which are not comparable
    ERROR,
}

//for any comparison other than equal
struct RefValueEvaluator {
    left: ParameterInstanceRef,
    right: Value,
    operator: ComparisonOperator,
}

//special type for equality comparison since this is most common and can be optimized a bit
struct RefEqualValueEvaluator {
    left: ParameterInstanceRef,
    right: Value,
}

pub(crate) fn from_comparison(
    mdb: &MissionDatabase,
    comp: &Comparison,
) -> Result<Box<dyn CriteriaEvaluator>> {
    let param = mdb.get_parameter(comp.param_instance.pidx);
    let ptypeidx = param.ptype.ok_or_else(|| ProcError::NoDataTypeAvailable(format!(
        "no type available for {}; without a type, the parameter cannot be used in a comparisoon",
        mdb.name2str(param.name())
    )))?;

    let mut ptype = mdb.get_data_type(ptypeidx);
    let param_instance = comp.param_instance.clone();
    if let Some(path) = &param_instance.member_path {
        if let Some(p) = get_member_type(mdb, ptype, path) {
            ptype = p;
        } else {
            return Err(ProcError::InvalidMdb(format!(
                "Cannot find parameter instance {}",
                param_instance.to_string(mdb)
            )));
        }
    }

    log::debug!(" Creating evaluator for {:?}", MdbItemDebug { mdb, item: comp });
    let right = ptype.from_str(&comp.value, param_instance.use_calibrated_value)?;

    if let ComparisonOperator::Equality = comp.comparison_operator {
        Ok(Box::new(RefEqualValueEvaluator { left: param_instance, right }))
    } else {
        Ok(Box::new(RefValueEvaluator {
            left: param_instance,
            operator: comp.comparison_operator,
            right,
        }))
    }
}

pub(crate) fn from_comparison_list(
    mdb: &MissionDatabase,
    clist: &Vec<Comparison>,
) -> Result<Box<dyn CriteriaEvaluator>> {
    let mut evlist = Vec::<Box<dyn CriteriaEvaluator>>::with_capacity(clist.len());
    for comp in clist {
        evlist.push(from_comparison(mdb, comp)?);
    }

    Ok(Box::new(AndEvaluator { list: evlist }))
}

impl CriteriaEvaluator for OrEvaluator {
    fn evaluate(&self, ctx: &ProcCtx) -> MatchResult {
        for m in &self.list {
            if m.evaluate(ctx) == MatchResult::OK {
                return MatchResult::OK;
            }
        }
        MatchResult::NOK
    }
}

impl CriteriaEvaluator for AndEvaluator {
    fn evaluate(&self, ctx: &ProcCtx) -> MatchResult {
        for m in &self.list {
            if m.evaluate(ctx) != MatchResult::OK {
                return MatchResult::NOK;
            }
        }
        MatchResult::OK
    }
}

//evaluator for equality comparisons
impl CriteriaEvaluator for RefEqualValueEvaluator {
    fn evaluate(&self, ctx: &ProcCtx) -> MatchResult {
        let left = ctx.get_param_value(&self.left);

        match left {
            Some(left) => compare_equal(&left, &self.right),
            None => MatchResult::UNDEF,
        }
    }
}

fn compare_equal(x: &Value, y: &Value) -> MatchResult {
    if discriminant(x) == discriminant(y) {
        //x and y are the same type
        return if x == y { MatchResult::OK } else { MatchResult::NOK };
    }

    //x and y are different types
    match (x, y) {
        (Value::Int64(x), Value::Uint64(y)) => check_equals(*x as i128, *y as i128),
        (Value::Uint64(x), Value::Int64(y)) => check_equals(*x as i128, *y as i128),
        (Value::Int64(x), Value::Double(y)) => check_equals(*x as f64, *y as f64),
        (Value::Double(x), Value::Int64(y)) => check_equals(*x as f64, *y as f64),
        (Value::Uint64(x), Value::Double(y)) => check_equals(*x as f64, *y as f64),
        (Value::Double(x), Value::Uint64(y)) => check_equals(*x as f64, *y as f64),
        (Value::StringValue(x), Value::Enumerated(y)) => check_equals(x.as_ref(), &y.value),
        (Value::Enumerated(x), Value::StringValue(y)) => check_equals(&x.value, y),

        //Yamcs java does some weird comparisons between different types
        _ => todo!(),
    }
}

fn check_equals<T: PartialEq>(x: T, y: T) -> MatchResult {
    return if x == y { MatchResult::OK } else { MatchResult::NOK };
}

//evaluator for other (>, >=,...) comparisons
impl CriteriaEvaluator for RefValueEvaluator {
    fn evaluate(&self, ctx: &ProcCtx) -> MatchResult {
        let left = ctx.get_param_value(&self.left);

        match left {
            Some(left) => compare(self.operator, &left, &self.right),
            None => MatchResult::UNDEF,
        }
    }
}

fn compare(operator: ComparisonOperator, x: &Value, y: &Value) -> MatchResult {
    match (x, y) {
        (Value::Int64(x), Value::Int64(y)) => compare_values(operator, *x as i128, *y as i128),
        (Value::Int64(x), Value::Uint64(y)) => compare_values(operator, *x as i128, *y as i128),
        (Value::Uint64(x), Value::Uint64(y)) => compare_values(operator, *x as i128, *y as i128),
        (Value::Uint64(x), Value::Int64(y)) => compare_values(operator, *x as i128, *y as i128),
        (Value::Double(x), Value::Double(y)) => compare_values(operator, *x as f64, *y as f64),
        (Value::Int64(x), Value::Double(y)) => compare_values(operator, *x as f64, *y as f64),
        (Value::Double(x), Value::Int64(y)) => compare_values(operator, *x as f64, *y as f64),
        (Value::Uint64(x), Value::Double(y)) => compare_values(operator, *x as f64, *y as f64),
        (Value::Double(x), Value::Uint64(y)) => compare_values(operator, *x as f64, *y as f64),
        (Value::StringValue(x), Value::Enumerated(y)) => {
            compare_values(operator, x.as_ref(), &y.value)
        }
        (Value::Enumerated(x), Value::StringValue(y)) => compare_values(operator, &x.value, y),

        //Yamcs java does some weird comparisons between different types
        _ => todo!(),
    }
}

fn compare_values<T: PartialEq + PartialOrd>(
    operator: ComparisonOperator,
    x: T,
    y: T,
) -> MatchResult {
    let b = match operator {
        ComparisonOperator::Equality => x == y,
        ComparisonOperator::Inequality => x != y,
        ComparisonOperator::LargerThan => x > y,
        ComparisonOperator::LargerOrEqualThan => x >= y,
        ComparisonOperator::SmallerThan => x < y,
        ComparisonOperator::SmallerOrEqualThan => x <= y,
    };

    if b {
        MatchResult::OK
    } else {
        MatchResult::NOK
    }
}
