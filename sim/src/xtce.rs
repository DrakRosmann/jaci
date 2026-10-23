//! Dicionário de telemetria CCSDS em XTCE (XML Telemetric and Command
//! Exchange, CCSDS 660.0 / OMG): cada SequenceContainer com o APID no critério
//! de restrição vira um pacote com layout fixo, e cada parâmetro um ponto de
//! telemetria com unidade, calibração e limites de alarme.
//!
//! Lê o subconjunto que um computador de bordo simples usa:
//!
//! * `ParameterTypeSet`: `FloatParameterType` e `IntegerParameterType`, com
//!   `UnitSet`, `FloatDataEncoding` (32 ou 64 bits) ou `IntegerDataEncoding`
//!   (8 a 64 bits, `encoding="unsigned"` ou com sinal), `PolynomialCalibrator`
//!   e `DefaultAlarm` / `StaticAlarmRanges` (`WarningRange` → amarelo,
//!   `CriticalRange` → vermelho);
//! * `ParameterSet`: os parâmetros, com `shortDescription` como nome e um
//!   `Alias nameSpace="jaci"` opcional como id no Jaci (ex. `nav.lat`, para o
//!   mapa);
//! * `ContainerSet`: `SequenceContainer` com `EntryList` de
//!   `ParameterRefEntry`, na ordem, e `RestrictionCriteria` com
//!   `Comparison parameterRef="CCSDS_APID" value="…"` (decimal ou 0x…).
//!
//! Os campos vêm em sequência, big-endian, logo depois do cabeçalho
//! secundário (o tempo CUC), sem preenchimento.

use serde::Serialize;

#[derive(Clone, Debug, PartialEq, Serialize)]
pub enum Encoding {
    Float(u32),
    Int { bits: u32, signed: bool },
}

#[derive(Clone, Debug, Serialize)]
pub struct FieldDef {
    /// Id no Jaci: o alias "jaci" ou o nome do parâmetro.
    pub id: String,
    pub name: String,
    pub unit: String,
    pub encoding: Encoding,
    /// Calibração polinomial: valor = Σ cᵢ·xⁱ.
    pub poly: Vec<f64>,
    pub yellow: Option<(f64, f64)>,
    pub red: Option<(f64, f64)>,
}

#[derive(Clone, Debug, Serialize)]
pub struct PacketDef {
    pub apid: u16,
    /// Nome do sistema (SpaceSystem) e do contêiner.
    pub system: String,
    pub container: String,
    pub fields: Vec<FieldDef>,
}

impl PacketDef {
    /// Tamanho dos dados (bytes) que o layout espera.
    pub fn size(&self) -> usize {
        self.fields.iter().map(|f| bits(&f.encoding) as usize / 8).sum()
    }

    /// Decodifica os dados de um pacote (sem cabeçalhos) nos pontos calibrados.
    pub fn decode(&self, data: &[u8]) -> Result<Vec<(String, f64)>, String> {
        if data.len() < self.size() {
            return Err(format!("APID 0x{:03X}: {} bytes, o layout de {} pede {}", self.apid, data.len(), self.container, self.size()));
        }
        let mut at = 0;
        let mut out = Vec::with_capacity(self.fields.len());
        for f in &self.fields {
            let n = bits(&f.encoding) as usize / 8;
            let b = &data[at..at + n];
            at += n;
            let raw = match f.encoding {
                Encoding::Float(32) => f32::from_be_bytes([b[0], b[1], b[2], b[3]]) as f64,
                Encoding::Float(_) => f64::from_be_bytes([b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7]]),
                Encoding::Int { bits, signed } => {
                    let mut u: u64 = 0;
                    for x in b {
                        u = (u << 8) | *x as u64;
                    }
                    if signed && bits < 64 && (u >> (bits - 1)) & 1 == 1 {
                        (u as i64 - (1i64 << bits)) as f64
                    } else if signed {
                        u as i64 as f64
                    } else {
                        u as f64
                    }
                }
            };
            let v = if f.poly.is_empty() { raw } else { f.poly.iter().enumerate().map(|(i, c)| c * raw.powi(i as i32)).sum() };
            out.push((f.id.clone(), v));
        }
        Ok(out)
    }

    /// Monta um pacote de dados com estes valores (os campos que faltam vão em zero).
    pub fn encode(&self, values: &[(&str, f64)]) -> Vec<u8> {
        let mut out = Vec::with_capacity(self.size());
        for f in &self.fields {
            let mut v = values.iter().find(|(k, _)| *k == f.id).map_or(0.0, |x| x.1);
            // Desfaz a calibração linear (a usada na prática para codificar).
            if f.poly.len() == 2 && f.poly[1] != 0.0 {
                v = (v - f.poly[0]) / f.poly[1];
            }
            match f.encoding {
                Encoding::Float(32) => out.extend_from_slice(&(v as f32).to_be_bytes()),
                Encoding::Float(_) => out.extend_from_slice(&v.to_be_bytes()),
                Encoding::Int { bits, .. } => {
                    let u = v.round() as i64 as u64;
                    let n = bits as usize / 8;
                    out.extend_from_slice(&u.to_be_bytes()[8 - n..]);
                }
            }
        }
        out
    }
}

fn bits(e: &Encoding) -> u32 {
    match e {
        Encoding::Float(b) => *b,
        Encoding::Int { bits, .. } => *bits,
    }
}

/// Tipo de parâmetro lido do XTCE.
struct TypeDef {
    unit: String,
    encoding: Encoding,
    poly: Vec<f64>,
    yellow: Option<(f64, f64)>,
    red: Option<(f64, f64)>,
}

fn num(s: &str) -> Option<f64> {
    let s = s.trim();
    match s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")) {
        Some(h) => u64::from_str_radix(h, 16).ok().map(|x| x as f64),
        None => s.parse().ok(),
    }
}

fn range(n: roxmltree::Node) -> Option<(f64, f64)> {
    let lo = ["minInclusive", "minExclusive"].iter().find_map(|a| n.attribute(*a)).and_then(num).unwrap_or(f64::NEG_INFINITY);
    let hi = ["maxInclusive", "maxExclusive"].iter().find_map(|a| n.attribute(*a)).and_then(num).unwrap_or(f64::INFINITY);
    Some((lo, hi))
}

/// Lê um documento XTCE; devolve os pacotes, ou o erro com o que faltou.
pub fn parse(xml: &str) -> Result<Vec<PacketDef>, String> {
    let doc = roxmltree::Document::parse(xml).map_err(|e| format!("XML: {e}"))?;
    let named = |n: &roxmltree::Node, tag: &str| n.is_element() && n.tag_name().name() == tag;
    let mut out = Vec::new();
    for sys in doc.descendants().filter(|n| named(n, "SpaceSystem")) {
        let system = sys.attribute("name").unwrap_or("XTCE").to_string();
        let Some(tm) = sys.children().find(|n| named(n, "TelemetryMetaData")) else { continue };
        // Tipos.
        let mut types: Vec<(String, TypeDef)> = Vec::new();
        for t in tm.children().filter(|n| named(n, "ParameterTypeSet")).flat_map(|s| s.children()).filter(|n| n.is_element()) {
            let Some(name) = t.attribute("name") else { continue };
            let unit = t.descendants().find(|n| named(n, "Unit")).and_then(|u| u.text()).unwrap_or("").trim().to_string();
            let encoding = match t.tag_name().name() {
                "FloatParameterType" => {
                    let b = t.descendants().find(|n| named(n, "FloatDataEncoding")).and_then(|e| e.attribute("sizeInBits")).and_then(num).unwrap_or(32.0) as u32;
                    if b != 32 && b != 64 {
                        return Err(format!("{name}: float de {b} bits (use 32 ou 64)"));
                    }
                    Encoding::Float(b)
                }
                "IntegerParameterType" => {
                    let e = t.descendants().find(|n| named(n, "IntegerDataEncoding"));
                    let b = e.and_then(|e| e.attribute("sizeInBits")).and_then(num).or_else(|| t.attribute("sizeInBits").and_then(num)).unwrap_or(32.0) as u32;
                    if ![8, 16, 24, 32, 64].contains(&b) {
                        return Err(format!("{name}: inteiro de {b} bits (use 8, 16, 24, 32 ou 64)"));
                    }
                    let signed = e.and_then(|e| e.attribute("encoding")).is_none_or(|x| x != "unsigned") && t.attribute("signed") != Some("false");
                    Encoding::Int { bits: b, signed }
                }
                _ => continue,
            };
            let poly: Vec<f64> = match t.descendants().find(|n| named(n, "PolynomialCalibrator")) {
                Some(p) => {
                    let mut terms: Vec<(usize, f64)> = p.children().filter(|n| named(n, "Term")).filter_map(|n| Some((n.attribute("exponent").and_then(num)? as usize, n.attribute("coefficient").and_then(num)?))).collect();
                    terms.sort_by_key(|x| x.0);
                    let deg = terms.last().map_or(0, |x| x.0);
                    let mut c = vec![0.0; deg + 1];
                    for (e, k) in terms {
                        c[e] += k;
                    }
                    c
                }
                None => vec![],
            };
            let ranges = t.descendants().find(|n| named(n, "StaticAlarmRanges"));
            let yellow = ranges.and_then(|r| r.children().find(|n| named(n, "WarningRange"))).and_then(range);
            let red = ranges.and_then(|r| r.children().find(|n| named(n, "CriticalRange"))).and_then(range);
            types.push((name.to_string(), TypeDef { unit, encoding, poly, yellow, red }));
        }
        // Parâmetros.
        let mut params: Vec<(String, FieldDef)> = Vec::new();
        for p in tm.children().filter(|n| named(n, "ParameterSet")).flat_map(|s| s.children()).filter(|n| named(n, "Parameter")) {
            let name = p.attribute("name").ok_or("Parameter sem name")?;
            let tref = p.attribute("parameterTypeRef").ok_or(format!("{name}: sem parameterTypeRef"))?;
            let t = types.iter().find(|(n, _)| n == tref).map(|x| &x.1).ok_or(format!("{name}: tipo {tref} não definido (ou não suportado)"))?;
            let alias = p.descendants().find(|n| named(n, "Alias") && n.attribute("nameSpace") == Some("jaci")).and_then(|a| a.attribute("alias"));
            let label = p.attribute("shortDescription").unwrap_or(name);
            params.push((
                name.to_string(),
                FieldDef { id: alias.unwrap_or(name).to_string(), name: label.to_string(), unit: t.unit.clone(), encoding: t.encoding.clone(), poly: t.poly.clone(), yellow: t.yellow, red: t.red },
            ));
        }
        // Contêineres com APID.
        for c in tm.children().filter(|n| named(n, "ContainerSet")).flat_map(|s| s.children()).filter(|n| named(n, "SequenceContainer")) {
            let container = c.attribute("name").unwrap_or("?").to_string();
            let apid = c.descendants().filter(|n| named(n, "Comparison")).find(|n| n.attribute("parameterRef").is_some_and(|r| r.ends_with("CCSDS_APID") || r.ends_with("APID"))).and_then(|n| n.attribute("value")).and_then(num);
            let Some(apid) = apid else { continue };
            if apid < 0.0 || apid > 2047.0 {
                return Err(format!("{container}: APID {apid} fora de 0–2047"));
            }
            let mut fields = Vec::new();
            for e in c.children().filter(|n| named(n, "EntryList")).flat_map(|l| l.children()).filter(|n| named(n, "ParameterRefEntry")) {
                let r = e.attribute("parameterRef").ok_or(format!("{container}: ParameterRefEntry sem parameterRef"))?;
                let f = params.iter().find(|(n, _)| n == r).map(|x| x.1.clone()).ok_or(format!("{container}: parâmetro {r} não definido"))?;
                fields.push(f);
            }
            if fields.is_empty() {
                return Err(format!("{container}: nenhum parâmetro no EntryList"));
            }
            out.push(PacketDef { apid: apid as u16, system: system.clone(), container, fields });
        }
    }
    if out.is_empty() {
        return Err("nenhum SequenceContainer com APID (RestrictionCriteria/Comparison CCSDS_APID)".into());
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    const XML: &str = include_str!("../../missoes/ccsds/cubesat.xml");

    #[test]
    fn le_o_exemplo_e_ida_e_volta() {
        let defs = parse(XML).unwrap();
        assert_eq!(defs.len(), 1);
        let d = &defs[0];
        assert_eq!(d.apid, 0x210);
        assert_eq!(d.system, "CubeSat-Jaci");
        let bat = d.fields.iter().find(|f| f.id == "eps.batt_v").unwrap();
        assert_eq!(bat.unit, "V");
        assert_eq!(bat.red, Some((6.0, 8.6)));
        assert!(d.fields.iter().any(|f| f.id == "nav.lat"));
        let vals = [("nav.lat", -2.5), ("nav.lon", -44.25), ("nav.alt", 512.0), ("eps.batt_v", 7.42), ("tcs.obc_temp", -12.5), ("eps.mode", 3.0)];
        let data = d.encode(&vals);
        assert_eq!(data.len(), d.size());
        let back = d.decode(&data).unwrap();
        for (k, v) in vals {
            let got = back.iter().find(|(id, _)| id == k).unwrap().1;
            assert!((got - v).abs() < 0.01, "{k}: {got} ≠ {v}");
        }
        assert!(d.decode(&data[..3]).is_err());
    }

    #[test]
    fn erros_claros() {
        assert!(parse("<x").unwrap_err().starts_with("XML"));
        assert!(parse("<SpaceSystem name=\"a\"/>").unwrap_err().contains("APID"));
        let sem_tipo = XML.replace("parameterTypeRef=\"Volts\"", "parameterTypeRef=\"Nada\"");
        assert!(parse(&sem_tipo).unwrap_err().contains("Nada"));
    }
}
