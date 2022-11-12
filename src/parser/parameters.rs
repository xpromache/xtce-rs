use std::str::FromStr;

use super::*;
use crate::mdb::*;



pub(super) fn add_parameter(mdb: &mut MissionDatabase, ctx: &ParseContext) -> Result<(), XtceError> {
    let node = &ctx.node;
    let ptype_str = read_mandatory_attribute::<String>(node, "parameterTypeRef")?;
    let rtype = NameReferenceType::ParameterType;
    let (qn, type_name) = match ctx.name_tree.resolve_ref(&ptype_str, ctx.path, rtype) {
        Some((qn, ptype_idx, _)) => (qn, ptype_idx),
        None => {
            return Err(XtceError::UndefinedReference(ptype_str, rtype));
        }
    };
    let mut ndescr = NameDescription::new(ctx.name);
    read_name_description(&mut ndescr, node);

    let ptype = mdb
        .get_parameter_type_idx(qn, type_name)
        .ok_or(XtceError::UnresolvedReference(ptype_str, rtype))?;
    let data_source = (read_attribute::<DataSource>(node, "dataSource")?).unwrap_or(DataSource::Telemetered);

    mdb.add_parameter(
        ctx.path,
        Parameter {
            ndescr,
            ptype: Some(ptype),
            data_source,
        },
    );

    Ok(())
}

impl FromStr for DataSource {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "Telemetered" => Ok(DataSource::Telemetered),
            "Derived" => Ok(DataSource::Derived),
            "Constant" => Ok(DataSource::Constant),
            "Local" => Ok(DataSource::Local),
            "System" => Ok(DataSource::System),
            _ => Err("please use one of Telemetered, Derived, Constant, Local or System".to_owned()),
        }
    }
}
