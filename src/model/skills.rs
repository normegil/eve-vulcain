use super::common::{Identified, Named};

#[derive(Debug, PartialEq, Clone)]
pub struct Skill {
    id: i32,
    name: String,
}

impl Skill {
    pub fn new(id: i32, name: &str) -> Self {
        Self {
            id,
            name: name.to_string(),
        }
    }
}

impl Identified<i32> for Skill {
    fn id(&self) -> i32 {
        self.id
    }
}

impl Named for Skill {
    fn name(&self) -> String {
        self.name.clone()
    }
}

#[derive(Debug, PartialEq, Clone)]
pub struct TrainedSkill {
    skill: Skill,
    pub trained_level: i32,
}

impl TrainedSkill {
    pub fn new(id: i32, name: &str, trained_level: i32) -> Self {
        Self {
            skill: Skill {
                id,
                name: name.to_string(),
            },
            trained_level,
        }
    }
}

impl Identified<i32> for TrainedSkill {
    fn id(&self) -> i32 {
        self.skill.id()
    }
}

impl Named for TrainedSkill {
    fn name(&self) -> String {
        self.skill.name()
    }
}
