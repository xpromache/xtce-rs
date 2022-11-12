use crate::mdb::*;
use core::fmt;
use std::fmt::Formatter;

struct MdbItemDebug<'a, T> {
    item: &'a T,
    mdb: &'a MissionDatabase,
}

impl std::fmt::Debug for MdbItemDebug<'_, SpaceSystem> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let mdb = self.mdb;
        let ss = self.item;

        let ssname = ss.fqn.to_string(mdb.name_db_ref());
        writeln!(f, "SpaceSystem {}", ssname)?;
        writeln!(f, "\t{} Containers: ", ss.containers.len())?;
        for (_, v) in &ss.containers {
            let pt = mdb.get_container(v.to_owned());
            writeln!(f, "\t\t {:?} ", MdbItemDebug { item: pt, mdb })?;
        }

        writeln!(f, "\t{} Parameter Types: ", ss.parameter_types.len())?;
        for (_, v) in &ss.parameter_types {
            let pt = mdb.get_parameter_type(v.to_owned());
            writeln!(f, "\t\t {:?} ", MdbItemDebug { item: pt, mdb })?;
        }
        writeln!(f, "\t{} Parameters: ", ss.parameters.len())?;
        for (_, v) in &ss.parameters {
            let pt = mdb.get_parameter(v.to_owned());
            writeln!(f, "\t\t {:?} ", MdbItemDebug { item: pt, mdb })?;
        }
        Ok(())
    }
}

fn write_debug_units(f: &mut Formatter<'_>, units: &Vec<UnitType>) -> fmt::Result {
    if units.len() > 0 {
        write!(f, ", units: {:?})", units)?;
    }
    Ok(())
}

impl std::fmt::Debug for MdbItemDebug<'_, ParameterType> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let mdb = self.mdb;
        match self.item {
            ParameterType::Integer(ipt) => {
                write!(f, "IntegerParameterType(name: {}, ", mdb.name2str(ipt.name.name))?;
                write!(f, "encoding: {:?}", self.item.encoding())?;
                write_debug_units(f, &ipt.units)?;
                write!(f, ")")?;
            }
            ParameterType::Float(fpt) => {
                write!(f, "FloatParameterType(name: {}, ", mdb.name2str(fpt.name.name))?;
                write!(f, "encoding: {:?}", self.item.encoding())?;
                write_debug_units(f, &fpt.units)?;
                write!(f, ")")?;
            }
            ParameterType::Enumerated(ept) => {
                write!(f, "EnumerateParameterType(name: {}, ", mdb.name2str(ept.name.name))?;
                write!(f, "encoding: {:?}, ", self.item.encoding())?;
                write!(f, "enumerations: {:?}", ept.enumeration)?;
                write_debug_units(f, &ept.units)?;
                write!(f, ")")?;
            }
            ParameterType::Boolean(bpt) => {
                write!(f, "BooleanParameterType(name: {}, ", mdb.name2str(bpt.name.name))?;
                write!(f, "encoding: {:?}, ", self.item.encoding())?;
                write!(
                    f,
                    "zeroStringValue: {}, oneStringValue: {}",
                    bpt.zero_string_value, bpt.one_string_value
                )?;
                write_debug_units(f, &bpt.units)?;
                write!(f, ")")?;
            }
            ParameterType::String(spt) => {
                write!(f, "StringParameterType(name: {}, ", mdb.name2str(spt.name.name))?;
                write!(f, "encoding: {:?}, ", self.item.encoding())?;
            }
            _ => {}
        };
        Ok(())
    }
}

impl std::fmt::Debug for MdbItemDebug<'_, Parameter> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let p = self.item;
        write!(f, "Parameter(name: {}, ", self.mdb.name2str(p.name()))?;
        if let Some(ptype_idx) = p.ptype {
            let ptype = self.mdb.get_parameter_type(ptype_idx);
            write!(f, "type: {}, ", self.mdb.name2str(ptype.name()))?;
        }
        write!(f, "data_source: {:?}", p.data_source)?;
        write!(f, ")")?;

        Ok(())
    }
}

impl std::fmt::Debug for MdbItemDebug<'_, SequenceContainer> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let p = self.item;
        write!(f, "SequenceContainer(name: {}, ", self.mdb.name2str(p.name()))?;
        write!(f, ")")?;

        Ok(())
    }
}

impl std::fmt::Debug for MdbItemDebug<'_, std::collections::HashMap<QualifiedName, Index>> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let mut first = true;
        for (k, v) in self.item {
            if first {
                first = false;
            } else {
                write!(f, ", ")?;
            }
            write!(f, "{}: {}", k.to_string(self.mdb.name_db_ref()), v.index())?;
        }

        Ok(())
    }
}

impl std::fmt::Debug for MissionDatabase {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        writeln!(f, "Mission Database with {} space systems", self.space_systems.len())?;
        for ss in &self.space_systems {
            writeln!(f, "{:?}", MdbItemDebug { item: ss, mdb: self })?;
        }
        Ok(())
    }
}
