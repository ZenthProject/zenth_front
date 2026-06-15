pub struct SecurityPassword;

impl SecurityPassword {

    pub fn check_username(username: &str) -> Result<(), String> {
        if username.is_empty() {
            return Err("Le nom d'utilisateur ne peut pas être vide.".to_string());
        }
        if username.chars().count() < 2 {
            return Err("Le nom d'utilisateur doit contenir au moins 2 caractères.".to_string());
        }
        if username.chars().count() > 40 {
            return Err("Le nom d'utilisateur ne doit pas dépasser 40 caractères.".to_string());
        }
        Ok(())
    }

    pub fn check_key(key: &str) -> Result<(), String> {
        if key.is_empty() {
            return Err("La clé ne peut pas être vide.".to_string());
        }
        if key.chars().count() < 19990 {
            return Err("La clé doit contenir au moins 19990 caractères.".to_string());
        }
        if key.chars().count() > 40000 {
            return Err("La clé ne doit pas dépasser 40000 caractères.".to_string());
        }
        Ok(())
    }

    
    fn has_min_count<F>(s: &str, predicate: F, min: usize) -> bool
    where
        F: Fn(char) -> bool,
    {
        s.chars().filter(|&c| predicate(c)).count() >= min
    }

    fn is_special(c: char) -> bool {
        !c.is_ascii_alphanumeric()
    }

    fn no_more_than_two_consecutive(s: &str) -> bool {
        let mut last_char = '\0';
        let mut count = 0;

        for c in s.chars() {
            if c == last_char {
                count += 1;
                if count >= 3 {
                    return false;
                }
            } else {
                last_char = c;
                count = 1;
            }
        }
        true
    }

    fn no_simple_sequences(s: &str) -> bool {
        let sequences = [
            "abcdefghijklmnopqrstuvwxyz",
            "ABCDEFGHIJKLMNOPQRSTUVWXYZ",
            "0123456789",
            "!@#$%^&*()-_=+[]{}|;:',.<>/?`~",
        ];

        let length = 4;
        let s_lower = s.to_lowercase();

        for seq in &sequences {
            for i in 0..=(seq.len() - length) {
                let forward_seq = &seq[i..i + length];
                let backward_seq: String = forward_seq.chars().rev().collect();

                if s_lower.contains(forward_seq) || s_lower.contains(&backward_seq) {
                    return false;
                }
            }
        }
        true
    }

    pub fn validate(password: &str) -> Result<(), String> {
        if password.chars().count() < 20 {
            return Err("Le mot de passe doit contenir au moins 20 caractères.".to_string());
        }
        if !Self::has_min_count(password, |c| c.is_uppercase(), 3) {
            return Err("Le mot de passe doit contenir au moins 3 majuscules.".to_string());
        }
        if !Self::has_min_count(password, |c| c.is_lowercase(), 3) {
            return Err("Le mot de passe doit contenir au moins 3 minuscules.".to_string());
        }
        if !Self::has_min_count(password, |c| char::is_ascii_digit(&c), 3) {
            return Err("Le mot de passe doit contenir au moins 3 chiffres.".to_string());
        }
        if !Self::has_min_count(password, Self::is_special, 3) {
            return Err("Le mot de passe doit contenir au moins 3 caractères spéciaux.".to_string());
        }
        if !Self::no_more_than_two_consecutive(password) {
            return Err("Le mot de passe ne doit pas contenir plus de 2 caractères identiques consécutifs.".to_string());
        }
        if !Self::no_simple_sequences(password) {
            return Err("Le mot de passe ne doit pas contenir de séquences simples comme abcd, 1234, dcba.".to_string());
        }
        Ok(())
    }

    pub fn check_register_params(password: &str, username: &str, key: &str) -> Result<(), String> {
        SecurityPassword::validate(password)?;
        SecurityPassword::check_username(username)?;
        SecurityPassword::check_key(key)?;
        Ok(())
    }

    /// Validates registration parameters without requiring a key
    /// (key is auto-generated at registration)
    pub fn check_register_params_no_key(password: &str, username: &str) -> Result<(), String> {
        SecurityPassword::validate(password)?;
        SecurityPassword::check_username(username)?;
        Ok(())
    }

}