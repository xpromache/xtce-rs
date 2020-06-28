use crate::mdb::*;
use core::fmt;
use std::fmt::Formatter;

struct MdbItemDebug<'a, T> {
    item: &'a T,
    mdb: &'a MissionDatabase,
}


impl std::fmt::Debug for MdbItemDebug<'_, SpaceSystem> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        writeln!(f, "SpaceSystem {:?}", self.mdb.qn_to_string(&self.item.fqn))?;
        writeln!(f, "\t{} Containers: ", self.item.sequence_containers.len())?;
        writeln!(f, "\t{} Parameter Types: ", self.item.parameter_types.len())?;
        for (k, v) in &self.item.parameter_types {
            let pt = self.mdb.get_parameter_type(v.to_owned()).unwrap();
            writeln!(f, "\t\t {:?} ", MdbItemDebug { item: pt, mdb: self.mdb })?;
        }
        writeln!(f, "\t{} Parameters: ", self.item.parameters.len())?;

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
        match self.item {
            ParameterType::None => { write!(f, "None ", )?; }
            ParameterType::Integer(ipt) => {
                write!(f, "IntegerParameterType(name: {}, ", self.mdb.name_to_string(ipt.name.name).unwrap())?;
                write!(f, "encoding: {:?}", self.item.encoding())?;
                write_debug_units(f, &ipt.units)?;
                write!(f, ")")?;
            }
            ParameterType::Float(fpt) => {
                write!(f, "FloatParameterType(name: {}, ", self.mdb.name_to_string(fpt.name.name).unwrap())?;
                write!(f, "encoding: {:?}", self.item.encoding())?;
                write_debug_units(f, &fpt.units)?;
                write!(f, ")")?;
            }
            ParameterType::Enumerated(ept) => {
                write!(f, "EnumerateParameterType(name: {}, ", self.mdb.name_to_string(ept.name.name).unwrap())?;
                write!(f, "encoding: {:?}, ", self.item.encoding())?;
                write!(f, "enumerations: {:?}", ept.enumeration)?;
                write_debug_units(f, &ept.units)?;
                write!(f, ")")?;
            }
            ParameterType::Boolean(bpt) => {
                write!(f, "BooleanParameterType(name: {}, ", self.mdb.name_to_string(bpt.name.name).unwrap())?;
                write!(f, "encoding: {:?}, ", self.item.encoding())?;
                write!(f, "zeroStringValue: {}, oneStringValue: {}", bpt.zero_string_value, bpt.one_string_value)?;
                write_debug_units(f, &bpt.units)?;
                write!(f, ")")?;
            }
            ParameterType::String(spt) => {
                write!(f, "StringParameterType(name: {}, ", self.mdb.name_to_string(spt.name.name).unwrap())?;
                write!(f, "encoding: {:?}, ", self.item.encoding())?;
            }
            _ => {}
        };
        Ok(())
    }
}


impl std::fmt::Debug for MdbItemDebug<'_, std::collections::HashMap<QualifiedName, Index>> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let mut first = true;
        for (k, v) in self.item {
            if (first) {
                first = false;
            } else {
                write!(f, ", ")?;
            }
            write!(f, "{}: {}", self.mdb.qn_to_string(k), v.index())?;
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