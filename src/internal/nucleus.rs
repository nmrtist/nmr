//! Shared normalization for portable nucleus labels.

use std::fmt::Write;

const VERIFIED_ALIASES: [(&str, &str); 3] =
    [("proton", "1H"), ("deuterium", "2H"), ("tritium", "3H")];

const ELEMENT_NAMES: &[(&str, &str)] = &[
    ("hydrogen", "H"),
    ("helium", "He"),
    ("lithium", "Li"),
    ("beryllium", "Be"),
    ("boron", "B"),
    ("carbon", "C"),
    ("nitrogen", "N"),
    ("oxygen", "O"),
    ("fluorine", "F"),
    ("neon", "Ne"),
    ("sodium", "Na"),
    ("magnesium", "Mg"),
    ("aluminum", "Al"),
    ("silicon", "Si"),
    ("phosphorus", "P"),
    ("sulfur", "S"),
    ("chlorine", "Cl"),
    ("argon", "Ar"),
    ("potassium", "K"),
    ("calcium", "Ca"),
    ("scandium", "Sc"),
    ("titanium", "Ti"),
    ("vanadium", "V"),
    ("chromium", "Cr"),
    ("manganese", "Mn"),
    ("iron", "Fe"),
    ("cobalt", "Co"),
    ("nickel", "Ni"),
    ("copper", "Cu"),
    ("zinc", "Zn"),
    ("gallium", "Ga"),
    ("germanium", "Ge"),
    ("arsenic", "As"),
    ("selenium", "Se"),
    ("bromine", "Br"),
    ("krypton", "Kr"),
    ("rubidium", "Rb"),
    ("strontium", "Sr"),
    ("yttrium", "Y"),
    ("zirconium", "Zr"),
    ("niobium", "Nb"),
    ("molybdenum", "Mo"),
    ("technetium", "Tc"),
    ("ruthenium", "Ru"),
    ("rhodium", "Rh"),
    ("palladium", "Pd"),
    ("silver", "Ag"),
    ("cadmium", "Cd"),
    ("indium", "In"),
    ("tin", "Sn"),
    ("antimony", "Sb"),
    ("tellurium", "Te"),
    ("iodine", "I"),
    ("xenon", "Xe"),
    ("cesium", "Cs"),
    ("barium", "Ba"),
    ("lanthanum", "La"),
    ("cerium", "Ce"),
    ("praseodymium", "Pr"),
    ("neodymium", "Nd"),
    ("promethium", "Pm"),
    ("samarium", "Sm"),
    ("europium", "Eu"),
    ("gadolinium", "Gd"),
    ("terbium", "Tb"),
    ("dysprosium", "Dy"),
    ("holmium", "Ho"),
    ("erbium", "Er"),
    ("thulium", "Tm"),
    ("ytterbium", "Yb"),
    ("lutetium", "Lu"),
    ("hafnium", "Hf"),
    ("tantalum", "Ta"),
    ("tungsten", "W"),
    ("rhenium", "Re"),
    ("osmium", "Os"),
    ("iridium", "Ir"),
    ("platinum", "Pt"),
    ("gold", "Au"),
    ("mercury", "Hg"),
    ("thallium", "Tl"),
    ("lead", "Pb"),
    ("bismuth", "Bi"),
    ("polonium", "Po"),
    ("astatine", "At"),
    ("radon", "Rn"),
    ("francium", "Fr"),
    ("radium", "Ra"),
    ("actinium", "Ac"),
    ("thorium", "Th"),
    ("protactinium", "Pa"),
    ("uranium", "U"),
    ("neptunium", "Np"),
    ("plutonium", "Pu"),
    ("americium", "Am"),
    ("curium", "Cm"),
    ("berkelium", "Bk"),
    ("californium", "Cf"),
    ("einsteinium", "Es"),
    ("fermium", "Fm"),
    ("mendelevium", "Md"),
    ("nobelium", "No"),
    ("lawrencium", "Lr"),
    ("rutherfordium", "Rf"),
    ("dubnium", "Db"),
    ("seaborgium", "Sg"),
    ("bohrium", "Bh"),
    ("hassium", "Hs"),
    ("meitnerium", "Mt"),
    ("darmstadtium", "Ds"),
    ("roentgenium", "Rg"),
    ("copernicium", "Cn"),
    ("nihonium", "Nh"),
    ("flerovium", "Fl"),
    ("moscovium", "Mc"),
    ("livermorium", "Lv"),
    ("tennessine", "Ts"),
    ("oganesson", "Og"),
];

pub(crate) fn normalize(value: Option<String>) -> Option<String> {
    value.and_then(|value| normalize_label(&value))
}

fn normalize_label(value: &str) -> Option<String> {
    let trimmed = value
        .trim()
        .trim_matches(|character| character == '<' || character == '>')
        .trim();
    if trimmed.is_empty() {
        return None;
    }

    // At most the original UTF-8 length; filtering never grows the text.
    let mut compact = String::with_capacity(trimmed.len());
    compact.extend(
        trimmed
            .chars()
            .filter(|character| !character.is_ascii_whitespace()),
    );
    let isotope = compact.strip_prefix('^').unwrap_or(&compact);
    if isotope.is_empty() || isotope.eq_ignore_ascii_case("off") {
        return None;
    }

    if let Some((_, canonical)) = VERIFIED_ALIASES
        .iter()
        .find(|(alias, _)| isotope.eq_ignore_ascii_case(alias))
    {
        return Some((*canonical).to_owned());
    }

    canonical_isotope(isotope).or_else(|| Some(trimmed.to_owned()))
}

fn canonical_isotope(value: &str) -> Option<String> {
    let leading_digits = value.bytes().take_while(u8::is_ascii_digit).count();
    let (mass, element) = if leading_digits > 0 {
        (&value[..leading_digits], &value[leading_digits..])
    } else {
        let trailing_digits = value.bytes().rev().take_while(u8::is_ascii_digit).count();
        if trailing_digits == 0 || trailing_digits == value.len() {
            return None;
        }
        let split = value.len() - trailing_digits;
        (&value[split..], &value[..split])
    };
    let mass = mass.parse::<u16>().ok().filter(|mass| *mass > 0)?;
    let element = portable_element_symbol(element)?;
    // u16 contributes at most five digits; all element symbols have <= 2 bytes.
    let mut canonical = String::with_capacity(7);
    write!(canonical, "{mass}{element}").expect("writing to String cannot fail");
    Some(canonical)
}

fn portable_element_symbol(value: &str) -> Option<&'static str> {
    ELEMENT_NAMES
        .iter()
        .find(|(name, symbol)| {
            value.eq_ignore_ascii_case(name) || value.eq_ignore_ascii_case(symbol)
        })
        .map(|(_, symbol)| *symbol)
        .or_else(|| {
            [("aluminium", "Al"), ("caesium", "Cs"), ("sulphur", "S")]
                .into_iter()
                .find(|(name, _)| value.eq_ignore_ascii_case(name))
                .map(|(_, symbol)| symbol)
        })
}

#[cfg(test)]
mod tests {
    use super::{ELEMENT_NAMES, normalize};

    #[test]
    fn normalization_has_bounded_buffers_for_long_and_unicode_labels() {
        assert!(ELEMENT_NAMES.iter().all(|(_, symbol)| symbol.len() <= 2));
        for (source, expected) in [
            (" ^ 6 5 5 3 5 C a ", "65535Ca"),
            ("27 ALUMINIUM", "27Al"),
            ("133 caesium", "133Cs"),
            ("32 SULPHUR", "32S"),
        ] {
            let result = normalize(Some(source.to_owned())).unwrap();
            assert_eq!(result, expected);
            assert_eq!(result.capacity(), 7);
        }
        let opaque = format!("未知 {} 65536Xx", "nucleus ".repeat(4096));
        let result = normalize(Some(opaque.clone())).unwrap();
        assert_eq!(result, opaque);
        assert_eq!(result.capacity(), opaque.len());
    }

    #[test]
    fn verified_source_forms_and_isotope_syntax_have_one_portable_form() {
        for (source_contract, source, expected) in [
            ("Bruker NUC1/AXNUC", "<1H>", "1H"),
            ("Varian tn", "H1", "1H"),
            ("Varian dn", "C13", "13C"),
            ("JEOL domain", "Proton", "1H"),
            ("JEOL domain", "Deuterium", "2H"),
            ("JEOL domain", "Tritium", "3H"),
            ("JEOL domain", "Carbon13", "13C"),
            ("JEOL domain", "Fluorine19", "19F"),
            ("JCAMP observe nucleus", "<^1H>", "1H"),
            ("portable isotope syntax", "si29", "29Si"),
            ("portable isotope syntax", "129XE", "129Xe"),
        ] {
            assert_eq!(
                normalize(Some(source.to_owned())).as_deref(),
                Some(expected),
                "source contract: {source_contract}"
            );
        }
    }

    #[test]
    fn every_element_name_and_symbol_accepts_mass_first_or_last() {
        assert_eq!(ELEMENT_NAMES.len(), 118);
        for &(name, symbol) in ELEMENT_NAMES {
            let expected = format!("123{symbol}");
            for source in [
                format!("{name}123"),
                format!("{symbol}123"),
                expected.clone(),
            ] {
                assert_eq!(
                    normalize(Some(source.clone())).as_deref(),
                    Some(expected.as_str()),
                    "source={source}"
                );
            }
        }
    }

    #[test]
    fn missing_and_unknown_labels_are_not_guessed() {
        for source in ["", "  ", "off", "< OFF >"] {
            assert_eq!(normalize(Some(source.to_owned())), None);
        }
        for source in ["mystery", "Xx99", "Proton decoupled"] {
            assert_eq!(normalize(Some(source.to_owned())).as_deref(), Some(source));
        }
    }
}
