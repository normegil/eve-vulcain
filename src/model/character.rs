use crate::model::common::{Identified, Named};
use crate::model::facility::Facility;
use crate::model::locations::SolarSystem;

use super::skills::TrainedSkill;

#[derive(Debug, PartialEq)]
pub struct Character {
    pub id: i32,
    pub name: String,
    pub isk: f64,
    pub location: CharacterLocation,
    pub corporation: Corporation,
    pub skills: Skills,
}

impl Character {
    pub fn new(
        id: i32,
        name: String,
        isk: f64,
        location: CharacterLocation,
        corporation: Corporation,
        skills: Skills,
    ) -> Self {
        Self {
            id,
            name,
            location,
            corporation,
            skills,
            isk,
        }
    }
}

impl Identified<i32> for Character {
    fn id(&self) -> i32 {
        self.id
    }
}

impl Named for Character {
    fn name(&self) -> String {
        self.name.clone()
    }
}

#[derive(Debug, PartialEq)]
pub enum CharacterLocation {
    Facility(Facility),
    Space(SolarSystem),
}

#[derive(Debug, PartialEq)]
pub struct Corporation {
    pub id: i32,
    pub name: String,
    pub alliance: Option<Alliance>,
}

impl Corporation {
    pub fn new(id: i32, name: String, alliance: Option<Alliance>) -> Self {
        Self { id, name, alliance }
    }
}

impl Identified<i32> for Corporation {
    fn id(&self) -> i32 {
        self.id
    }
}

impl Named for Corporation {
    fn name(&self) -> String {
        self.name.clone()
    }
}

#[derive(Debug, PartialEq)]
pub struct Alliance {
    pub id: i32,
    pub name: String,
}

impl Alliance {
    pub fn new(id: i32, name: String) -> Self {
        Self { id, name }
    }
}

impl Identified<i32> for Alliance {
    fn id(&self) -> i32 {
        self.id
    }
}

impl Named for Alliance {
    fn name(&self) -> String {
        self.name.clone()
    }
}

#[derive(Debug, PartialEq)]
pub struct Skills {
    pub skills: Vec<TrainedSkill>,
}

impl Skills {
    pub fn get_manufacturing_skill(&self) -> ManufacturingSkills {
        let mut industry_skill = None;
        let mut advanced_industry_skill = None;
        for skill in &self.skills {
            if skill.name() == "Industry" {
                industry_skill = Some(skill.clone())
            }
            if skill.name() == "Advanced Industry" {
                advanced_industry_skill = Some(skill.clone())
            }
        }

        ManufacturingSkills {
            industry: industry_skill,
            advanced_industry_level: advanced_industry_skill,
        }
    }

    pub fn get_skill(&self, id: i32) -> Option<&TrainedSkill> {
        self.skills.iter().find(|&skill| skill.id() == id)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ManufacturingSkills {
    pub industry: Option<TrainedSkill>,
    pub advanced_industry_level: Option<TrainedSkill>,
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_get_manufacturing_skill_empty_skills() {
        let skills = Skills { skills: vec![] };
        let manufacturing_skills = skills.get_manufacturing_skill();

        assert_eq!(
            manufacturing_skills,
            ManufacturingSkills {
                industry: None,
                advanced_industry_level: None
            }
        );
    }

    #[test]
    fn test_get_manufacturing_skill_no_matching_skills() {
        let trained_skills = vec![
            TrainedSkill::new(1, "Combat", 5),
            TrainedSkill::new(2, "Science", 3),
        ];
        let skills = Skills {
            skills: trained_skills,
        };
        let manufacturing_skills = skills.get_manufacturing_skill();

        assert_eq!(
            manufacturing_skills,
            ManufacturingSkills {
                industry: None,
                advanced_industry_level: None
            }
        );
    }

    #[test]
    fn test_get_manufacturing_skill_with_matching_skills() {
        let trained_skills = vec![
            TrainedSkill::new(1, "Industry", 3),
            TrainedSkill::new(2, "Advanced Industry", 5),
        ];
        let skills = Skills {
            skills: trained_skills,
        };
        let manufacturing_skills = skills.get_manufacturing_skill();

        assert_eq!(
            manufacturing_skills,
            ManufacturingSkills {
                industry: Some(TrainedSkill::new(1, "Industry", 3)),
                advanced_industry_level: Some(TrainedSkill::new(2, "Advanced Industry", 5)),
            }
        );
    }

    #[test]
    fn test_get_skill_empty_skills() {
        let skills = Skills { skills: vec![] };
        let skill = skills.get_skill(1);

        assert_eq!(skill, None);
    }

    #[test]
    fn test_get_skill_skill_not_found() {
        let trained_skills = vec![
            TrainedSkill::new(1, "Combat", 5),
            TrainedSkill::new(2, "Science", 3),
        ];
        let skills = Skills {
            skills: trained_skills,
        };
        let skill = skills.get_skill(3);

        assert_eq!(skill, None);
    }

    #[test]
    fn test_get_skill_skill_found() {
        let trained_skills = vec![
            TrainedSkill::new(1, "Industry", 3),
            TrainedSkill::new(2, "Advanced Industry", 5),
        ];
        let skills = Skills {
            skills: trained_skills,
        };
        let skill = skills.get_skill(2);

        assert_eq!(skill, Some(&TrainedSkill::new(2, "Advanced Industry", 5)));
    }
}
