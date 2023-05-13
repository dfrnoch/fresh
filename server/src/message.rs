use common::proto::Env;
use smallvec::SmallVec;

const ENVS_SIZE: usize = 8;

pub struct Envs(SmallVec<[Env; ENVS_SIZE]>);

impl Envs {
    pub fn new0() -> Envs {
        let sv: SmallVec<[Env; ENVS_SIZE]> = SmallVec::new();
        Envs(sv)
    }

    pub fn new1(e: Env) -> Envs {
        let mut sv: SmallVec<[Env; ENVS_SIZE]> = SmallVec::new();
        sv.push(e);
        Envs(sv)
    }

    pub fn new2(e0: Env, e1: Env) -> Envs {
        let mut sv: SmallVec<[Env; ENVS_SIZE]> = SmallVec::new();
        sv.push(e0);
        sv.push(e1);
        Envs(sv)
    }
}

impl AsRef<SmallVec<[Env; ENVS_SIZE]>> for Envs {
    fn as_ref(&self) -> &SmallVec<[Env; ENVS_SIZE]> {
        &self.0
    }
}

impl AsMut<SmallVec<[Env; ENVS_SIZE]>> for Envs {
    fn as_mut(&mut self) -> &mut SmallVec<[Env; ENVS_SIZE]> {
        &mut self.0
    }
}
