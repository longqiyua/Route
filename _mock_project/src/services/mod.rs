// Services module

pub struct UserService {
    users: Vec<super::models::User>,
}

impl UserService {
    pub fn new() -> Self {
        Self { users: Vec::new() }
    }

    pub fn create_user(&mut self, id: String, name: String, email: String) {
        self.users.push(super::models::User {
            id,
            name,
            email,
            active: true,
        });
    }

    pub fn get_user(&self, id: &str) -> Option<&super::models::User> {
        self.users.iter().find(|u| u.id == id)
    }

    pub fn update_user(&mut self, id: &str, name: Option<String>, email: Option<String>) {
        if let Some(user) = self.users.iter_mut().find(|u| u.id == id) {
            if let Some(n) = name { user.name = n; }
            if let Some(e) = email { user.email = e; }
        }
    }

    pub fn delete_user(&mut self, id: &str) {
        self.users.retain(|u| u.id != id);
    }
}