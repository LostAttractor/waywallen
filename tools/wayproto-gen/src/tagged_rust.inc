
mod tagged {
    use super::{wire, DecodeError};

    pub fn encode(buf: &mut Vec<u8>, mut fields: Vec<(u32, u32, Vec<u8>)>) {
        fields.sort_by_key(|field| field.0);
        wire::w_u32(buf, 1);
        let size = fields.iter().map(|field| 12 + field.2.len()).sum::<usize>();
        assert!(fields.len() <= 256 && size <= 65520, "tagged body exceeds limits");
        wire::w_u32(buf, u32::try_from(size).expect("tagged body exceeds u32"));
        for (tag, kind, value) in fields {
            wire::w_u32(buf, tag);
            wire::w_u32(buf, kind);
            wire::w_u32(buf, value.len() as u32);
            buf.extend_from_slice(&value);
        }
    }

    pub struct Fields<'a>(std::collections::BTreeMap<u32, (u32, &'a [u8])>);
    impl<'a> Fields<'a> {
        pub fn decode(reader: &mut wire::R<'a>) -> Result<Self, DecodeError> {
            let revision = reader.u32()?;
            let len = reader.u32()? as usize;
            if len > 65520 || len % 4 != 0 { return Err(DecodeError::BadArrayLen); }
            let body = reader.take(len)?;
            let mut fields = std::collections::BTreeMap::new();
            // Unknown revisions can only produce an empty optional proposal.
            if revision != 1 { return Ok(Self(fields)); }
            let mut r = wire::R::new(body);
            while !r.at_end() {
                if fields.len() == 256 { return Err(DecodeError::BadArrayLen); }
                let tag = r.u32()?;
                let kind = r.u32()?;
                let len = r.u32()? as usize;
                if tag == 0 || len % 4 != 0 { return Err(DecodeError::BadArrayLen); }
                let value = r.take(len)?;
                if fields.insert(tag, (kind, value)).is_some() {
                    return Err(DecodeError::BadArrayLen);
                }
            }
            Ok(Self(fields))
        }

        pub fn take(&mut self, tag: u32, kind: u32) -> Result<Option<wire::R<'a>>, DecodeError> {
            match self.0.remove(&tag) {
                Some((actual, value)) if actual == kind => {
                    validate(kind, &mut wire::R::new(value))?;
                    Ok(Some(wire::R::new(value)))
                }
                Some(_) => Err(DecodeError::BadArrayLen),
                None => Ok(None),
            }
        }
    }

    fn validate(kind: u32, r: &mut wire::R<'_>) -> Result<(), DecodeError> {
        if kind >= 0x100 || kind == 9 {
            let n = r.u32()? as usize;
            if n > 1024 { return Err(DecodeError::BadArrayLen); }
            let element = if kind == 9 { 8 } else { kind - 0x100 };
            for _ in 0..n {
                validate(element, r)?;
                if kind == 9 { validate(8, r)?; }
            }
        } else if kind == 8 {
            let len = r.u32()? as usize;
            if len == 0 || len > 4097 { return Err(DecodeError::BadString); }
            let bytes = r.take((len + 3) & !3)?;
            if bytes[..len - 1].contains(&0) || bytes[len - 1..].iter().any(|b| *b != 0) {
                return Err(DecodeError::BadString);
            }
            std::str::from_utf8(&bytes[..len - 1]).map_err(|_| DecodeError::BadString)?;
        } else if kind == 11 {
            let _revision = r.u32()?;
            let len = r.u32()? as usize;
            r.take(len)?;
        } else {
            r.take(match kind { 4 | 5 | 7 => 8, 10 => 16, _ => 4 })?;
        }
        Ok(())
    }
}
