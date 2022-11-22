use crate::{
    mdb::{Comparison, ComparisonOperator, MissionDatabase, NamedItem, ParameterInstanceRef, debug::MdbItemDebug},
    value::ValueUnion,
};

use super::{MdbError, ProcCtx};

pub(crate) trait CriteriaEvaluator {
    fn evaluate(&self, ctx: &ProcCtx) -> bool;
}

struct OrEvaluator {
    list: Vec<Box<dyn CriteriaEvaluator>>,
}

struct AndEvaluator {
    list: Vec<Box<dyn CriteriaEvaluator>>,
}

enum Operand {
    Constant(ValueUnion<()>),
    ParamRef(ParameterInstanceRef),
}
struct ExprEvaluator {
    left: Operand,
    operator: ComparisonOperator,
    right: Operand,
}

pub(crate) fn from_comparison(
    mdb: &MissionDatabase,
    comp: &Comparison,
) -> Result<Box<dyn CriteriaEvaluator>, MdbError> {
    let param = mdb.get_parameter(comp.param_instance.pidx);
    let ptypeidx = param.ptype.ok_or(MdbError::NoDataTypeAvailable(format!(
        "no type available for {}; without a type, the parameter cannot be used in a comparisoon",
        mdb.name2str(param.name())
    )))?;

    let ptype = mdb.get_data_type(ptypeidx);
    let param_instance = comp.param_instance.clone();


    log::debug!(" Creating evaluator for {:?}", MdbItemDebug{mdb, item: comp});
    let v = ptype.from_str(&comp.value, param_instance.use_calibrated_value)?;

    let exprev = ExprEvaluator {
        left: Operand::ParamRef(param_instance),
        operator: comp.comparison_operator.clone(),
        right: Operand::Constant(v),
    };

    Ok(Box::new(exprev))
}

pub(crate) fn from_comparison_list(
    mdb: &MissionDatabase,
    clist: &Vec<Comparison>,
) -> Result<Box<dyn CriteriaEvaluator>, MdbError> {
    let mut evlist = Vec::<Box<dyn CriteriaEvaluator>>::with_capacity(clist.len());
    for comp in clist {
        evlist.push(from_comparison(mdb, comp)?);
    }

    Ok(Box::new(AndEvaluator { list: evlist }))
}

impl CriteriaEvaluator for ExprEvaluator {
    fn evaluate(&self, ctx: &ProcCtx) -> bool {
        todo!()
    }
}

impl CriteriaEvaluator for OrEvaluator {
    fn evaluate(&self, ctx: &ProcCtx) -> bool {
        for m in &self.list {
            if m.evaluate(ctx) {
                return true;
            }
        }
        false
    }
}

impl CriteriaEvaluator for AndEvaluator {
    fn evaluate(&self, ctx: &ProcCtx) -> bool {
        for m in &self.list {
            if m.evaluate(ctx) {
                return false;
            }
        }
        true
    }
}
