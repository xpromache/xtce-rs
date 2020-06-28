use super::*;
use crate::mdb::*;

use roxmltree::{TextPos, Node, NodeId, Document};


pub(super) fn add_parameter(mdb: &mut MissionDatabase, ctx: &ParseContext) -> Result<(), XtceParseError> {
    let node = &ctx.node;
    let ptype_str = read_mandatory_attribute::<String>(node, "parameterTypeRef")?;
    let ptype_idx = mdb.resolve_reference(NameReferenceType::ParameterType, &ptype_str, ctx.path);
   // let type_ss_qn = name_tree.resolve

    if (ptype_idx == None) {
        return Ok(());
    }

    for cnode in node.children() {
        match cnode.tag_name().name() {
            "" => {}
            _ => println!("ignoring read_parameter '{}'", cnode.tag_name().name())
        };
    }

    Ok(())
}
