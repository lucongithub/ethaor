//! Decoupage du texte sous le chapitre : versets, mots, lettres.
//!
//! Tout est calcule a la demande, uniquement pour ce qui est visible, et
//! toujours **de droite a gauche**. L'alignement hebreu est donc exact par
//! construction : on ne demande jamais son avis au moteur de texte.
/// Une lettre = une consonne et les marques qui la suivent (niqqud,
/// cantillation). Les marques sont des caracteres combinants : les detacher de
/// leur consonne casserait le rendu.
pub fn letters(word: &str) -> Letters<'_> {
    Letters { rest: word }
}
/// `Clone` est necessaire : on parcourt deux fois, une fois pour la somme des
/// largeurs, une fois pour poser les cellules.
#[derive(Clone)]
pub struct Letters<'a> {
    rest: &'a str,
}
impl<'a> Iterator for Letters<'a> {
    type Item = &'a str;
    fn next(&mut self) -> Option<&'a str> {
        if self.rest.is_empty() {
            return None;
        }
        let end = self
            .rest
            .char_indices()
            .skip(1)
            .find(|(_, c)| is_base(*c))
            .map_or(self.rest.len(), |(i, _)| i);
        let (head, tail) = self.rest.split_at(end);
        self.rest = tail;
        Some(head)
    }
}
/// Consonne hebraique, formes finales comprises.
fn is_base(c: char) -> bool {
    ('\u{05D0}'..='\u{05EA}').contains(&c)
}
pub fn letter_count(s: &str) -> usize {
    s.chars().filter(|&c| is_base(c)).count()
}
