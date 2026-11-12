//! Parsing of the algorithms. Only MathAlgorithm is supported.

use super::{
    misc::{read_para_insta_ref, resolve_para_ref},
    types::read_math_operation,
    utils::*,
    ParseContext, Result,
};
use crate::mdb::{types::MathOpElement, MathAlgorithm, MissionDatabase, ParameterInstanceRef};

pub(super) fn add_algorithm(mdb: &mut MissionDatabase, ctx: &ParseContext) -> Result<()> {
    let node = &ctx.node;
    if node.tag_name().name() != "MathAlgorithm" {
        log::warn!(
            "ignoring algorithm {}: {} is not supported (only MathAlgorithm is)",
            mdb.name2str(ctx.name),
            node.tag_name().name()
        );
        return Ok(());
    }

    let op_node = children(node)
        .find(|n| n.tag_name().name() == "MathOperation")
        .ok_or_else(|| get_parse_error("MathAlgorithm without MathOperation", node))?;

    let out_ref = read_mandatory_attribute::<String>(&op_node, "outputParameterRef")?;
    let (output, path) = resolve_para_ref(mdb, ctx, &out_ref)?;
    if path.is_some() {
        return Err(get_parse_error("aggregate members cannot be used as algorithm output", &op_node));
    }

    let mut inputs: Vec<ParameterInstanceRef> = Vec::new();
    let expression = read_math_operation(&op_node, &mut |cnode| {
        inputs.push(read_para_insta_ref(mdb, ctx, cnode, false)?);
        Ok(MathOpElement::Input(inputs.len() - 1))
    })?;

    let mut triggers = Vec::new();
    for tsnode in children(&op_node).filter(|n| n.tag_name().name() == "TriggerSet") {
        for tnode in children(&tsnode) {
            match tnode.tag_name().name() {
                "OnParameterUpdateTrigger" => {
                    let pref = read_mandatory_attribute::<String>(&tnode, "parameterRef")?;
                    triggers.push(resolve_para_ref(mdb, ctx, &pref)?.0);
                }
                name => log::warn!(
                    "algorithm {}: ignoring unsupported trigger {}",
                    mdb.name2str(ctx.name),
                    name
                ),
            }
        }
    }

    let ndescr = read_name_description(ctx);
    mdb.add_algorithm(ctx.path, MathAlgorithm { ndescr, output, inputs, expression, triggers });
    Ok(())
}
