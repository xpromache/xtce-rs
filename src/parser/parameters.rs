use std::str::FromStr;

use super::{*, misc::resolve_ref};
use crate::mdb::*;



pub(super) fn add_parameter(mdb: &mut MissionDatabase, ctx: &ParseContext) -> Result<()> {
    let node = &ctx.node;
    let ptype_str = read_mandatory_attribute::<String>(node, "parameterTypeRef")?;
    let rtype = NameReferenceType::ParameterType;

    let type_idx = resolve_ref(mdb, ctx, &ptype_str, rtype)?;
    let ndescr = read_name_description(ctx);

    
    // dataSource is an attribute of ParameterProperties (XTCE 1.1 and 1.2); it was an attribute of the parameter in some older files
    let mut data_source =
        (read_attribute::<DataSource>(node, "dataSource")?).unwrap_or(DataSource::Telemetered);
    for cnode in node.children() {
        match cnode.tag_name().name() {
            "ParameterProperties" => {
                if let Some(ds) = read_attribute::<DataSource>(&cnode, "dataSource")? {
                    data_source = ds;
                }
            }
            "" | "LongDescription" | "AliasSet" | "AncillaryDataSet" => {}
            name => log::warn!("ignoring parameter unknown property '{}'", name),
        }
    }

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
    type Err = XtceError;

    fn from_str(s: &str) -> Result<Self> {
        match s.to_lowercase().as_str() {
            "telemetered" => Ok(DataSource::Telemetered),
            "derived" => Ok(DataSource::Derived),
            "constant" => Ok(DataSource::Constant),
            "local" => Ok(DataSource::Local),
            "system" => Ok(DataSource::System),
            "command" => Ok(DataSource::Command),
            "commandhistory" => Ok(DataSource::CommandHistory),
            "external1" => Ok(DataSource::External1),
            "external2" => Ok(DataSource::External2),
            "external3" => Ok(DataSource::External3),
            _ => Err(XtceError::InvalidValue(
                "please use one of telemetered, derived, constant, local or system".to_owned(),
            )),
        }
    }
}
