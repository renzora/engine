use syn::{Field, Lit, Type};

/// Parsed attributes from `#[field(...)]` on a struct field.
pub struct FieldAttrs {
    pub name: Option<String>,
    pub speed: Option<f32>,
    pub min: Option<f32>,
    pub max: Option<f32>,
    pub default: Option<f64>,
    pub skip: bool,
    pub readonly: bool,
}

impl Default for FieldAttrs {
    fn default() -> Self {
        Self {
            name: None,
            speed: None,
            min: None,
            max: None,
            default: None,
            skip: false,
            readonly: false,
        }
    }
}

impl FieldAttrs {
    pub fn from_field(field: &Field) -> syn::Result<Self> {
        let mut attrs = Self::default();
        for attr in &field.attrs {
            if !attr.path().is_ident("field") {
                continue;
            }
            attr.parse_nested_meta(|meta| {
                if meta.path.is_ident("skip") {
                    attrs.skip = true;
                } else if meta.path.is_ident("readonly") {
                    attrs.readonly = true;
                } else if meta.path.is_ident("name") {
                    let value = meta.value()?;
                    let lit: Lit = value.parse()?;
                    if let Lit::Str(s) = lit {
                        attrs.name = Some(s.value());
                    }
                } else if meta.path.is_ident("speed") {
                    let value = meta.value()?;
                    let lit: Lit = value.parse()?;
                    if let Lit::Float(f) = &lit {
                        attrs.speed = Some(f.base10_parse()?);
                    } else if let Lit::Int(i) = &lit {
                        attrs.speed = Some(i.base10_parse()?);
                    }
                } else if meta.path.is_ident("min") {
                    let value = meta.value()?;
                    let lit: Lit = value.parse()?;
                    if let Lit::Float(f) = &lit {
                        attrs.min = Some(f.base10_parse()?);
                    } else if let Lit::Int(i) = &lit {
                        attrs.min = Some(i.base10_parse()?);
                    }
                } else if meta.path.is_ident("default") {
                    let value = meta.value()?;
                    let lit: Lit = value.parse()?;
                    if let Lit::Float(f) = &lit {
                        attrs.default = Some(f.base10_parse()?);
                    } else if let Lit::Int(i) = &lit {
                        attrs.default = Some(i.base10_parse()?);
                    }
                } else if meta.path.is_ident("max") {
                    let value = meta.value()?;
                    let lit: Lit = value.parse()?;
                    if let Lit::Float(f) = &lit {
                        attrs.max = Some(f.base10_parse()?);
                    } else if let Lit::Int(i) = &lit {
                        attrs.max = Some(i.base10_parse()?);
                    }
                }
                Ok(())
            })?;
        }
        Ok(attrs)
    }
}

/// Convert a field name like `my_field_name` to `"My Field Name"`.
pub fn title_case(s: &str) -> String {
    s.split('_')
        .filter(|w| !w.is_empty())
        .map(|w| {
            let mut c = w.chars();
            match c.next() {
                None => String::new(),
                Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Infer the FieldType variant from a Rust type path.
/// Returns a string like "Float", "Bool", "Vec3", "String", or "ReadOnly".
pub fn infer_field_type(ty: &Type) -> &'static str {
    if let Type::Path(tp) = ty {
        if let Some(seg) = tp.path.segments.last() {
            let name = seg.ident.to_string();
            return match name.as_str() {
                "f32" | "f64" => "Float",
                "bool" => "Bool",
                "Vec3" => "Vec3",
                "String" => "String",
                "Color" => "Color",
                _ => "ReadOnly",
            };
        }
    }
    "ReadOnly"
}
