// Models module

pub struct User {
    pub id: String,
    pub name: String,
    pub email: String,
    pub active: bool,
}

pub struct Product {
    pub id: u64,
    pub name: String,
    pub price: f64,
    pub category: String,
}

pub struct Order {
    pub id: u64,
    pub user_id: String,
    pub product_ids: Vec<u64>,
    pub total: f64,
}