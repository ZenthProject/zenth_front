/// Emergency numbers by country
/// Provides police, medical, fire and crisis hotlines

use std::collections::HashMap;

#[derive(Debug, Clone, serde::Serialize)]
pub struct EmergencyNumbers {
    pub country_code: String,
    pub country_name: String,
    pub police: String,
    pub medical: String,
    pub fire: String,
    pub general: Option<String>,        // Unified emergency number (like 112)
    pub crisis_hotline: Option<String>, // Mental health / suicide prevention
}

pub fn get_emergency_numbers() -> HashMap<String, EmergencyNumbers> {
    let mut numbers = HashMap::new();

    // Europe
    numbers.insert("FR".to_string(), EmergencyNumbers {
        country_code: "FR".to_string(),
        country_name: "France".to_string(),
        police: "17".to_string(),
        medical: "15".to_string(),
        fire: "18".to_string(),
        general: Some("112".to_string()),
        crisis_hotline: Some("3114".to_string()), // Numéro national de prévention du suicide
    });

    numbers.insert("DE".to_string(), EmergencyNumbers {
        country_code: "DE".to_string(),
        country_name: "Germany".to_string(),
        police: "110".to_string(),
        medical: "112".to_string(),
        fire: "112".to_string(),
        general: Some("112".to_string()),
        crisis_hotline: Some("0800-1110111".to_string()),
    });

    numbers.insert("GB".to_string(), EmergencyNumbers {
        country_code: "GB".to_string(),
        country_name: "United Kingdom".to_string(),
        police: "999".to_string(),
        medical: "999".to_string(),
        fire: "999".to_string(),
        general: Some("112".to_string()),
        crisis_hotline: Some("116123".to_string()), // Samaritans
    });

    numbers.insert("ES".to_string(), EmergencyNumbers {
        country_code: "ES".to_string(),
        country_name: "Spain".to_string(),
        police: "091".to_string(),
        medical: "061".to_string(),
        fire: "080".to_string(),
        general: Some("112".to_string()),
        crisis_hotline: Some("024".to_string()),
    });

    numbers.insert("IT".to_string(), EmergencyNumbers {
        country_code: "IT".to_string(),
        country_name: "Italy".to_string(),
        police: "113".to_string(),
        medical: "118".to_string(),
        fire: "115".to_string(),
        general: Some("112".to_string()),
        crisis_hotline: Some("800-274274".to_string()),
    });

    numbers.insert("PT".to_string(), EmergencyNumbers {
        country_code: "PT".to_string(),
        country_name: "Portugal".to_string(),
        police: "112".to_string(),
        medical: "112".to_string(),
        fire: "112".to_string(),
        general: Some("112".to_string()),
        crisis_hotline: Some("808-200-204".to_string()),
    });

    numbers.insert("NL".to_string(), EmergencyNumbers {
        country_code: "NL".to_string(),
        country_name: "Netherlands".to_string(),
        police: "112".to_string(),
        medical: "112".to_string(),
        fire: "112".to_string(),
        general: Some("112".to_string()),
        crisis_hotline: Some("113".to_string()),
    });

    numbers.insert("BE".to_string(), EmergencyNumbers {
        country_code: "BE".to_string(),
        country_name: "Belgium".to_string(),
        police: "101".to_string(),
        medical: "100".to_string(),
        fire: "100".to_string(),
        general: Some("112".to_string()),
        crisis_hotline: Some("1813".to_string()),
    });

    numbers.insert("CH".to_string(), EmergencyNumbers {
        country_code: "CH".to_string(),
        country_name: "Switzerland".to_string(),
        police: "117".to_string(),
        medical: "144".to_string(),
        fire: "118".to_string(),
        general: Some("112".to_string()),
        crisis_hotline: Some("143".to_string()),
    });

    numbers.insert("AT".to_string(), EmergencyNumbers {
        country_code: "AT".to_string(),
        country_name: "Austria".to_string(),
        police: "133".to_string(),
        medical: "144".to_string(),
        fire: "122".to_string(),
        general: Some("112".to_string()),
        crisis_hotline: Some("142".to_string()),
    });

    numbers.insert("PL".to_string(), EmergencyNumbers {
        country_code: "PL".to_string(),
        country_name: "Poland".to_string(),
        police: "997".to_string(),
        medical: "999".to_string(),
        fire: "998".to_string(),
        general: Some("112".to_string()),
        crisis_hotline: Some("116123".to_string()),
    });

    numbers.insert("SE".to_string(), EmergencyNumbers {
        country_code: "SE".to_string(),
        country_name: "Sweden".to_string(),
        police: "112".to_string(),
        medical: "112".to_string(),
        fire: "112".to_string(),
        general: Some("112".to_string()),
        crisis_hotline: Some("90101".to_string()),
    });

    numbers.insert("NO".to_string(), EmergencyNumbers {
        country_code: "NO".to_string(),
        country_name: "Norway".to_string(),
        police: "112".to_string(),
        medical: "113".to_string(),
        fire: "110".to_string(),
        general: Some("112".to_string()),
        crisis_hotline: Some("116123".to_string()),
    });

    numbers.insert("DK".to_string(), EmergencyNumbers {
        country_code: "DK".to_string(),
        country_name: "Denmark".to_string(),
        police: "112".to_string(),
        medical: "112".to_string(),
        fire: "112".to_string(),
        general: Some("112".to_string()),
        crisis_hotline: Some("70201201".to_string()),
    });

    numbers.insert("FI".to_string(), EmergencyNumbers {
        country_code: "FI".to_string(),
        country_name: "Finland".to_string(),
        police: "112".to_string(),
        medical: "112".to_string(),
        fire: "112".to_string(),
        general: Some("112".to_string()),
        crisis_hotline: Some("09-2525-0111".to_string()),
    });

    numbers.insert("IE".to_string(), EmergencyNumbers {
        country_code: "IE".to_string(),
        country_name: "Ireland".to_string(),
        police: "999".to_string(),
        medical: "999".to_string(),
        fire: "999".to_string(),
        general: Some("112".to_string()),
        crisis_hotline: Some("116123".to_string()),
    });

    numbers.insert("GR".to_string(), EmergencyNumbers {
        country_code: "GR".to_string(),
        country_name: "Greece".to_string(),
        police: "100".to_string(),
        medical: "166".to_string(),
        fire: "199".to_string(),
        general: Some("112".to_string()),
        crisis_hotline: Some("1018".to_string()),
    });

    numbers.insert("CZ".to_string(), EmergencyNumbers {
        country_code: "CZ".to_string(),
        country_name: "Czech Republic".to_string(),
        police: "158".to_string(),
        medical: "155".to_string(),
        fire: "150".to_string(),
        general: Some("112".to_string()),
        crisis_hotline: Some("116123".to_string()),
    });

    numbers.insert("RO".to_string(), EmergencyNumbers {
        country_code: "RO".to_string(),
        country_name: "Romania".to_string(),
        police: "112".to_string(),
        medical: "112".to_string(),
        fire: "112".to_string(),
        general: Some("112".to_string()),
        crisis_hotline: Some("0800-801200".to_string()),
    });

    numbers.insert("HU".to_string(), EmergencyNumbers {
        country_code: "HU".to_string(),
        country_name: "Hungary".to_string(),
        police: "107".to_string(),
        medical: "104".to_string(),
        fire: "105".to_string(),
        general: Some("112".to_string()),
        crisis_hotline: Some("116123".to_string()),
    });

    numbers.insert("UA".to_string(), EmergencyNumbers {
        country_code: "UA".to_string(),
        country_name: "Ukraine".to_string(),
        police: "102".to_string(),
        medical: "103".to_string(),
        fire: "101".to_string(),
        general: Some("112".to_string()),
        crisis_hotline: Some("7333".to_string()),
    });

    numbers.insert("RU".to_string(), EmergencyNumbers {
        country_code: "RU".to_string(),
        country_name: "Russia".to_string(),
        police: "102".to_string(),
        medical: "103".to_string(),
        fire: "101".to_string(),
        general: Some("112".to_string()),
        crisis_hotline: Some("8-800-2000-122".to_string()),
    });

    // North America
    numbers.insert("US".to_string(), EmergencyNumbers {
        country_code: "US".to_string(),
        country_name: "United States".to_string(),
        police: "911".to_string(),
        medical: "911".to_string(),
        fire: "911".to_string(),
        general: Some("911".to_string()),
        crisis_hotline: Some("988".to_string()), // Suicide & Crisis Lifeline
    });

    numbers.insert("CA".to_string(), EmergencyNumbers {
        country_code: "CA".to_string(),
        country_name: "Canada".to_string(),
        police: "911".to_string(),
        medical: "911".to_string(),
        fire: "911".to_string(),
        general: Some("911".to_string()),
        crisis_hotline: Some("988".to_string()),
    });

    numbers.insert("MX".to_string(), EmergencyNumbers {
        country_code: "MX".to_string(),
        country_name: "Mexico".to_string(),
        police: "911".to_string(),
        medical: "911".to_string(),
        fire: "911".to_string(),
        general: Some("911".to_string()),
        crisis_hotline: Some("800-290-0024".to_string()),
    });

    // South America
    numbers.insert("BR".to_string(), EmergencyNumbers {
        country_code: "BR".to_string(),
        country_name: "Brazil".to_string(),
        police: "190".to_string(),
        medical: "192".to_string(),
        fire: "193".to_string(),
        general: None,
        crisis_hotline: Some("188".to_string()), // CVV
    });

    numbers.insert("AR".to_string(), EmergencyNumbers {
        country_code: "AR".to_string(),
        country_name: "Argentina".to_string(),
        police: "101".to_string(),
        medical: "107".to_string(),
        fire: "100".to_string(),
        general: None,
        crisis_hotline: Some("135".to_string()),
    });

    numbers.insert("CL".to_string(), EmergencyNumbers {
        country_code: "CL".to_string(),
        country_name: "Chile".to_string(),
        police: "133".to_string(),
        medical: "131".to_string(),
        fire: "132".to_string(),
        general: None,
        crisis_hotline: Some("600-360-7777".to_string()),
    });

    numbers.insert("CO".to_string(), EmergencyNumbers {
        country_code: "CO".to_string(),
        country_name: "Colombia".to_string(),
        police: "123".to_string(),
        medical: "123".to_string(),
        fire: "123".to_string(),
        general: Some("123".to_string()),
        crisis_hotline: Some("106".to_string()),
    });

    numbers.insert("PE".to_string(), EmergencyNumbers {
        country_code: "PE".to_string(),
        country_name: "Peru".to_string(),
        police: "105".to_string(),
        medical: "117".to_string(),
        fire: "116".to_string(),
        general: None,
        crisis_hotline: Some("0800-00-0068".to_string()),
    });

    // Asia
    numbers.insert("JP".to_string(), EmergencyNumbers {
        country_code: "JP".to_string(),
        country_name: "Japan".to_string(),
        police: "110".to_string(),
        medical: "119".to_string(),
        fire: "119".to_string(),
        general: None,
        crisis_hotline: Some("0120-783-556".to_string()),
    });

    numbers.insert("CN".to_string(), EmergencyNumbers {
        country_code: "CN".to_string(),
        country_name: "China".to_string(),
        police: "110".to_string(),
        medical: "120".to_string(),
        fire: "119".to_string(),
        general: None,
        crisis_hotline: Some("400-161-9995".to_string()),
    });

    numbers.insert("KR".to_string(), EmergencyNumbers {
        country_code: "KR".to_string(),
        country_name: "South Korea".to_string(),
        police: "112".to_string(),
        medical: "119".to_string(),
        fire: "119".to_string(),
        general: None,
        crisis_hotline: Some("1393".to_string()),
    });

    numbers.insert("IN".to_string(), EmergencyNumbers {
        country_code: "IN".to_string(),
        country_name: "India".to_string(),
        police: "100".to_string(),
        medical: "102".to_string(),
        fire: "101".to_string(),
        general: Some("112".to_string()),
        crisis_hotline: Some("9152987821".to_string()), // iCall
    });

    numbers.insert("TH".to_string(), EmergencyNumbers {
        country_code: "TH".to_string(),
        country_name: "Thailand".to_string(),
        police: "191".to_string(),
        medical: "1669".to_string(),
        fire: "199".to_string(),
        general: None,
        crisis_hotline: Some("1323".to_string()),
    });

    numbers.insert("VN".to_string(), EmergencyNumbers {
        country_code: "VN".to_string(),
        country_name: "Vietnam".to_string(),
        police: "113".to_string(),
        medical: "115".to_string(),
        fire: "114".to_string(),
        general: None,
        crisis_hotline: None,
    });

    numbers.insert("PH".to_string(), EmergencyNumbers {
        country_code: "PH".to_string(),
        country_name: "Philippines".to_string(),
        police: "911".to_string(),
        medical: "911".to_string(),
        fire: "911".to_string(),
        general: Some("911".to_string()),
        crisis_hotline: Some("1553".to_string()),
    });

    numbers.insert("ID".to_string(), EmergencyNumbers {
        country_code: "ID".to_string(),
        country_name: "Indonesia".to_string(),
        police: "110".to_string(),
        medical: "118".to_string(),
        fire: "113".to_string(),
        general: Some("112".to_string()),
        crisis_hotline: Some("119".to_string()),
    });

    numbers.insert("MY".to_string(), EmergencyNumbers {
        country_code: "MY".to_string(),
        country_name: "Malaysia".to_string(),
        police: "999".to_string(),
        medical: "999".to_string(),
        fire: "994".to_string(),
        general: Some("112".to_string()),
        crisis_hotline: Some("03-7956-8145".to_string()),
    });

    numbers.insert("SG".to_string(), EmergencyNumbers {
        country_code: "SG".to_string(),
        country_name: "Singapore".to_string(),
        police: "999".to_string(),
        medical: "995".to_string(),
        fire: "995".to_string(),
        general: None,
        crisis_hotline: Some("1800-221-4444".to_string()),
    });

    numbers.insert("HK".to_string(), EmergencyNumbers {
        country_code: "HK".to_string(),
        country_name: "Hong Kong".to_string(),
        police: "999".to_string(),
        medical: "999".to_string(),
        fire: "999".to_string(),
        general: Some("999".to_string()),
        crisis_hotline: Some("2382-0000".to_string()),
    });

    numbers.insert("TW".to_string(), EmergencyNumbers {
        country_code: "TW".to_string(),
        country_name: "Taiwan".to_string(),
        police: "110".to_string(),
        medical: "119".to_string(),
        fire: "119".to_string(),
        general: None,
        crisis_hotline: Some("1925".to_string()),
    });

    numbers.insert("IL".to_string(), EmergencyNumbers {
        country_code: "IL".to_string(),
        country_name: "Israel".to_string(),
        police: "100".to_string(),
        medical: "101".to_string(),
        fire: "102".to_string(),
        general: None,
        crisis_hotline: Some("1201".to_string()),
    });

    numbers.insert("AE".to_string(), EmergencyNumbers {
        country_code: "AE".to_string(),
        country_name: "United Arab Emirates".to_string(),
        police: "999".to_string(),
        medical: "998".to_string(),
        fire: "997".to_string(),
        general: Some("112".to_string()),
        crisis_hotline: Some("800-4673".to_string()),
    });

    numbers.insert("SA".to_string(), EmergencyNumbers {
        country_code: "SA".to_string(),
        country_name: "Saudi Arabia".to_string(),
        police: "999".to_string(),
        medical: "997".to_string(),
        fire: "998".to_string(),
        general: Some("911".to_string()),
        crisis_hotline: None,
    });

    numbers.insert("TR".to_string(), EmergencyNumbers {
        country_code: "TR".to_string(),
        country_name: "Turkey".to_string(),
        police: "155".to_string(),
        medical: "112".to_string(),
        fire: "110".to_string(),
        general: Some("112".to_string()),
        crisis_hotline: Some("182".to_string()),
    });

    // Oceania
    numbers.insert("AU".to_string(), EmergencyNumbers {
        country_code: "AU".to_string(),
        country_name: "Australia".to_string(),
        police: "000".to_string(),
        medical: "000".to_string(),
        fire: "000".to_string(),
        general: Some("000".to_string()),
        crisis_hotline: Some("13-11-14".to_string()), // Lifeline
    });

    numbers.insert("NZ".to_string(), EmergencyNumbers {
        country_code: "NZ".to_string(),
        country_name: "New Zealand".to_string(),
        police: "111".to_string(),
        medical: "111".to_string(),
        fire: "111".to_string(),
        general: Some("111".to_string()),
        crisis_hotline: Some("1737".to_string()),
    });

    // Africa
    numbers.insert("ZA".to_string(), EmergencyNumbers {
        country_code: "ZA".to_string(),
        country_name: "South Africa".to_string(),
        police: "10111".to_string(),
        medical: "10177".to_string(),
        fire: "10177".to_string(),
        general: Some("112".to_string()),
        crisis_hotline: Some("0800-567-567".to_string()),
    });

    numbers.insert("EG".to_string(), EmergencyNumbers {
        country_code: "EG".to_string(),
        country_name: "Egypt".to_string(),
        police: "122".to_string(),
        medical: "123".to_string(),
        fire: "180".to_string(),
        general: None,
        crisis_hotline: Some("08008880700".to_string()),
    });

    numbers.insert("MA".to_string(), EmergencyNumbers {
        country_code: "MA".to_string(),
        country_name: "Morocco".to_string(),
        police: "19".to_string(),
        medical: "15".to_string(),
        fire: "15".to_string(),
        general: None,
        crisis_hotline: None,
    });

    numbers.insert("NG".to_string(), EmergencyNumbers {
        country_code: "NG".to_string(),
        country_name: "Nigeria".to_string(),
        police: "112".to_string(),
        medical: "112".to_string(),
        fire: "112".to_string(),
        general: Some("112".to_string()),
        crisis_hotline: Some("0800-800-2000".to_string()),
    });

    numbers.insert("KE".to_string(), EmergencyNumbers {
        country_code: "KE".to_string(),
        country_name: "Kenya".to_string(),
        police: "999".to_string(),
        medical: "999".to_string(),
        fire: "999".to_string(),
        general: Some("112".to_string()),
        crisis_hotline: Some("0800-723-253".to_string()),
    });

    numbers
}

/// Get emergency numbers for a specific country
pub fn get_country_emergency(country_code: &str) -> Option<EmergencyNumbers> {
    get_emergency_numbers().get(&country_code.to_uppercase()).cloned()
}

/// Get all country codes available
pub fn get_available_countries() -> Vec<String> {
    let mut countries: Vec<String> = get_emergency_numbers().keys().cloned().collect();
    countries.sort();
    countries
}

// ============ Tauri Commands ============

#[tauri::command]
pub fn get_emergency_by_country(country_code: String) -> Result<EmergencyNumbers, String> {
    get_country_emergency(&country_code)
        .ok_or_else(|| format!("Country code '{}' not found", country_code))
}

#[tauri::command]
pub fn get_all_emergency_numbers() -> std::collections::HashMap<String, EmergencyNumbers> {
    get_emergency_numbers()
}

#[tauri::command]
pub fn list_emergency_countries() -> Vec<String> {
    get_available_countries()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_france() {
        let fr = get_country_emergency("FR").unwrap();
        assert_eq!(fr.police, "17");
        assert_eq!(fr.medical, "15");
        assert_eq!(fr.general, Some("112".to_string()));
    }

    #[test]
    fn test_get_us() {
        let us = get_country_emergency("US").unwrap();
        assert_eq!(us.general, Some("911".to_string()));
        assert_eq!(us.crisis_hotline, Some("988".to_string()));
    }

    #[test]
    fn test_case_insensitive() {
        assert!(get_country_emergency("fr").is_some());
        assert!(get_country_emergency("FR").is_some());
        assert!(get_country_emergency("Fr").is_some());
    }
}
