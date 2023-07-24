use std::{io::Read, iter};

use anyhow::{Context, Result};
use rusqlite::{types::ToSqlOutput, Connection, ToSql, Transaction};

use crate::sii::{
    parser::{Block, DataBlock, Parser, StructDef},
    value::{EncodedString, Value, ID},
};

macro_rules! quoted {
    ($e:expr) => {
        format!("\"{}\"", $e)
    };
}

pub fn copy_to_sqlite<R: Read>(mut parser: Parser<R>, conn: &mut Connection) -> Result<()> {
    let tx = conn.transaction()?;

    loop {
        match parser.next_block()? {
            None => break,
            Some(Block::Struct(struct_def)) => create_table(&tx, &struct_def)?,
            Some(Block::Data(data)) => insert_struct(&tx, &data)?,
        }
    }

    tx.commit()?;
    Ok(())
}

fn create_table(tx: &Transaction, s: &StructDef) -> Result<()> {
    let fields = iter::once(quoted!("struct_id"))
        .chain(s.fields.iter().map(|f| quoted!(f.name)))
        .collect::<Vec<String>>();

    let stmt = format!("CREATE TABLE {} ({})", s.name, fields.join(", "));
    tx.execute(&stmt, ())?;
    Ok(())
}

fn insert_struct(tx: &Transaction, data: &DataBlock) -> Result<()> {
    let mut fields = vec![quoted!("struct_id")];
    let mut params = vec!["?"];
    let id = Value::ID(data.id.clone());
    let mut bindings: Vec<&Value> = vec![&id];

    for (k, v) in &data.fields {
        fields.push(quoted!(k));
        params.push("?");
        bindings.push(v);
    }

    let query = format!(
        "INSERT INTO {} ({}) VALUES ({})",
        data.struct_name,
        fields.join(", "),
        params.join(", ")
    );
    let bound = rusqlite::params_from_iter(bindings.iter());
    tx.execute(&query, bound)
        .context(format!("when inserting {:?}", &data))?;

    Ok(())
}

impl ToSql for ID {
    fn to_sql(&self) -> rusqlite::Result<ToSqlOutput<'_>> {
        Ok(ToSqlOutput::from(self.to_string()))
    }
}

impl ToSql for EncodedString {
    fn to_sql(&self) -> rusqlite::Result<ToSqlOutput<'_>> {
        Ok(ToSqlOutput::from(self.to_string()))
    }
}

fn json_string_array<S: ToString>(v: &Vec<S>) -> String {
    let items = v
        .iter()
        .map(|e| quoted!(e.to_string()))
        .collect::<Vec<String>>()
        .join(",");

    format!("[{}]", items)
}

fn json_numeric_array<N: ToString>(v: &Vec<N>) -> String {
    let items = v
        .iter()
        .map(|e| e.to_string())
        .collect::<Vec<String>>()
        .join(",");

    format!("[{}]", items)
}

impl ToSql for Value {
    fn to_sql(&self) -> rusqlite::Result<rusqlite::types::ToSqlOutput<'_>> {
        let null = ToSqlOutput::Owned(rusqlite::types::Value::Null);
        match self {
            Value::String(s) => s.to_sql(),
            Value::StringArray(a) => Ok(ToSqlOutput::from(json_string_array(a))),
            Value::EncodedString(s) => s.to_sql(),
            Value::EncodedStringArray(a) => Ok(ToSqlOutput::from(json_string_array(a))),
            Value::Single(v) => v.to_sql(),
            Value::SingleArray(a) => Ok(ToSqlOutput::from(json_numeric_array(a))),
            // TODO: vecs not supported
            Value::Vec2s(_) => Ok(null),
            Value::Vec3s(_) => Ok(null),
            Value::Vec3sArray(_) => Ok(null),
            Value::Vec3i(_) => Ok(null),
            Value::Vec3iArray(_) => Ok(null),
            Value::Vec4s(_) => Ok(null),
            Value::Vec4sArray(_) => Ok(null),
            Value::Vec8s(_) => Ok(null),
            Value::Vec8sArray(_) => Ok(null),
            // end vecs
            Value::Int32(v) => v.to_sql(),
            Value::Int32Array(a) => Ok(ToSqlOutput::from(json_numeric_array(a))),
            Value::UInt32(v) => v.to_sql(),
            Value::UInt32Array(a) => Ok(ToSqlOutput::from(json_numeric_array(a))),
            Value::UInt16(v) => v.to_sql(),
            Value::UInt16Array(a) => Ok(ToSqlOutput::from(json_numeric_array(a))),
            Value::Int64(v) => v.to_sql(),
            Value::Int64Array(a) => Ok(ToSqlOutput::from(json_numeric_array(a))),
            // TODO: because of online_job_id: 18446744073709551615
            Value::UInt64(v) => Ok(ToSqlOutput::from(*v as i64)),
            Value::UInt64Array(a) => Ok(ToSqlOutput::from(json_numeric_array(a))),
            Value::ByteBool(b) => b.to_sql(),
            Value::ByteBoolArray(a) => Ok(ToSqlOutput::from(json_numeric_array(a))),
            Value::OrdinalString(s) => s.to_sql(),
            Value::ID(v) => v.to_sql(),
            Value::IDArray(a) => Ok(ToSqlOutput::from(json_string_array(a))),
        }
    }
}
