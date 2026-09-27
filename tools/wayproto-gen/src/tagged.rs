use std::fmt::Write;

use crate::codegen_rust::{
    emit_decode_arg, emit_encode_arg, field_name, rust_type, snake_to_camel,
};
use crate::parser::{Arg, ArgType, NamedStruct};

pub fn wire_type(ty: &ArgType) -> u32 {
    match ty {
        ArgType::Bool => 1,
        ArgType::U32 => 2,
        ArgType::I32 => 3,
        ArgType::U64 => 4,
        ArgType::I64 => 5,
        ArgType::F32 => 6,
        ArgType::F64 => 7,
        ArgType::String => 8,
        ArgType::KvList => 9,
        ArgType::Rect => 10,
        ArgType::Named(_) => 11,
        ArgType::Enum(_) => 12,
        ArgType::Array(element) => 0x100 + wire_type(element),
    }
}

pub fn rust_encode(out: &mut String, args: &[Arg], prefix: &str, indent: &str) {
    writeln!(out, "{indent}let mut fields = Vec::new();").unwrap();
    for arg in args {
        let name = field_name(&arg.name);
        let value = if prefix.is_empty() {
            name.clone()
        } else {
            format!("&{prefix}{name}")
        };
        if arg.optional {
            writeln!(out, "{indent}if let Some(value) = {value} {{").unwrap();
        } else {
            writeln!(out, "{indent}{{ let value = {value};").unwrap();
        }
        writeln!(
            out,
            "{indent}let mut payload = Vec::new();\n{indent}let buf = &mut payload;"
        )
        .unwrap();
        emit_encode_arg(out, indent, "value", &arg.ty);
        writeln!(
            out,
            "{indent}fields.push(({}, {}, payload));\n{indent}}}",
            arg.tag.unwrap(),
            wire_type(&arg.ty)
        )
        .unwrap();
    }
    writeln!(out, "{indent}tagged::encode(buf, fields);").unwrap();
}

pub fn rust_decode(out: &mut String, args: &[Arg], reader: &str, indent: &str) {
    writeln!(
        out,
        "{indent}let mut fields = tagged::Fields::decode({reader})?;"
    )
    .unwrap();
    for arg in args {
        let name = field_name(&arg.name);
        writeln!(
            out,
            "{indent}let {name} = if let Some(mut field_reader) = fields.take({}, {})? {{",
            arg.tag.unwrap(),
            wire_type(&arg.ty)
        )
        .unwrap();
        emit_decode_arg(out, indent, "value", &arg.ty, "&mut field_reader");
        writeln!(
            out,
            "{indent}if !field_reader.at_end() {{ return Err(DecodeError::Trailing); }}"
        )
        .unwrap();
        if arg.optional {
            writeln!(out, "{indent}Some(value)\n{indent}}} else {{ None }};").unwrap();
        } else {
            writeln!(
                out,
                "{indent}value\n{indent}}} else {{ return Err(DecodeError::TooShort); }};"
            )
            .unwrap();
        }
    }
}

pub fn rust_struct(out: &mut String, item: &NamedStruct) {
    let name = snake_to_camel(&item.name);
    writeln!(
        out,
        "#[derive(Debug, Clone, PartialEq)]\npub struct {name} {{"
    )
    .unwrap();
    for field in &item.fields {
        let ty = rust_type(&field.ty);
        writeln!(
            out,
            "    pub {}: {},",
            field_name(&field.name),
            if field.optional {
                format!("Option<{ty}>")
            } else {
                ty
            }
        )
        .unwrap();
    }
    writeln!(
        out,
        "}}\nimpl {name} {{\n    fn encode_wire(&self, buf: &mut Vec<u8>) {{"
    )
    .unwrap();
    rust_encode(out, &item.fields, "self.", "        ");
    writeln!(
        out,
        "    }}\n    fn decode_wire(reader: &mut wire::R<'_>) -> Result<Self, DecodeError> {{"
    )
    .unwrap();
    rust_decode(out, &item.fields, "reader", "        ");
    writeln!(
        out,
        "        Ok(Self {{ {} }})\n    }}\n}}",
        item.fields
            .iter()
            .map(|f| field_name(&f.name))
            .collect::<Vec<_>>()
            .join(", ")
    )
    .unwrap();
}

pub fn c_encode(out: &mut String, args: &[Arg], value: &str, buf: &str) {
    use crate::codegen_c::{c_field_name, encode_call_for};
    writeln!(out, "    if ((rc = w_u32({buf}, 1))) return rc;\n    size_t body_length_pos = {buf}->len;\n    if ((rc = w_u32({buf}, 0))) return rc;").unwrap();
    let mut fields: Vec<_> = args.iter().collect();
    fields.sort_by_key(|field| field.tag);
    for arg in fields {
        let name = c_field_name(&arg.name);
        let call = encode_call_for(&arg.ty, &name);
        let expr = match arg.ty {
            ArgType::Array(_) | ArgType::KvList | ArgType::Rect | ArgType::Named(_) => {
                format!("&{value}->{name}")
            }
            _ => format!("{value}->{name}"),
        };
        if arg.optional {
            writeln!(out, "    if ({value}->has_{name}) {{").unwrap();
        } else {
            writeln!(out, "    {{").unwrap();
        }
        writeln!(out, "        if ((rc = w_u32({buf}, {}))) return rc;\n        if ((rc = w_u32({buf}, {}))) return rc;\n        size_t length_pos = {buf}->len;\n        if ((rc = w_u32({buf}, 0))) return rc;\n        if ((rc = {call}({buf}, {expr}))) return rc;\n        if ({buf}->len - length_pos - 4 > UINT32_MAX) return WW_ERR_OVERFLOW;\n        tagged_patch_u32({buf}, length_pos, (uint32_t)({buf}->len - length_pos - 4));\n    }}", arg.tag.unwrap(), wire_type(&arg.ty)).unwrap();
    }
    writeln!(out, "    if ({buf}->len - body_length_pos - 4 > 65520) return WW_ERR_OVERFLOW;\n    tagged_patch_u32({buf}, body_length_pos, (uint32_t)({buf}->len - body_length_pos - 4));").unwrap();
}

pub fn c_decode(out: &mut String, args: &[Arg], value: &str, reader: &str) {
    use crate::codegen_c::{c_field_name, decode_call_for};
    writeln!(out, "    ww_tagged_reader_t fields;\n    if ((rc = tagged_begin({reader}, &fields))) return rc;").unwrap();
    for arg in args.iter().filter(|arg| !arg.optional) {
        writeln!(out, "    bool seen_{} = false;", c_field_name(&arg.name)).unwrap();
    }
    writeln!(out, "    while (fields.body.pos != fields.body.len) {{\n        uint32_t tag, kind;\n        ww_rd_t field;\n        if ((rc = tagged_next(&fields, &tag, &kind, &field))) return rc;\n        switch (tag) {{").unwrap();
    for arg in args {
        let name = c_field_name(&arg.name);
        let call = decode_call_for(&arg.ty);
        writeln!(out, "        case {}:\n            if (kind != {}) return WW_ERR_BAD_ARRAY;\n            if ((rc = tagged_validate(kind, field))) return rc;\n            if ((rc = {call}(&field, &{value}->{name}))) return rc;\n            if (field.pos != field.len) return WW_ERR_TRAILING;", arg.tag.unwrap(), wire_type(&arg.ty)).unwrap();
        if arg.optional {
            writeln!(out, "            {value}->has_{name} = true;").unwrap();
        } else {
            writeln!(out, "            seen_{name} = true;").unwrap();
        }
        writeln!(out, "            break;").unwrap();
    }
    writeln!(out, "        default: break;\n        }}\n    }}").unwrap();
    for arg in args.iter().filter(|arg| !arg.optional) {
        writeln!(
            out,
            "    if (!seen_{}) return WW_ERR_SHORT;",
            c_field_name(&arg.name)
        )
        .unwrap();
    }
}
