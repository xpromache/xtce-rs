use std::str::FromStr;

use super::*;
use crate::mdb::*;



pub(super) fn add_parameter(mdb: &mut MissionDatabase, ctx: &ParseContext) -> Result<(), XtceError> {
    let node = &ctx.node;
    let ptype_str = read_mandatory_attribute::<String>(node, "parameterTypeRef")?;
    let rtype = NameReferenceType::ParameterType;

    let type_idx = resolve_ref(mdb, ctx, &ptype_str, rtype)?;
    let mut ndescr = read_name_description(ctx);

    
    let data_source = (read_attribute::<DataSource>(node, "dataSource")?).unwrap_or(DataSource::Telemetered);

    mdb.add_parameter(
        ctx.path,
        Parameter {
            ndescr,
            ptype: Some(type_idx),
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
