# Lingerie E-commerce Website

A modern lingerie independent online store built with Rust and Axum.

## Features

- 🛍️ Product catalog with categories
- 🛒 Shopping cart management
- 👤 User authentication (JWT)
- 📦 Order management
- 🔐 Admin dashboard
- 📱 Responsive design
- 💳 Payment integration ready (Stripe/PayPal)

## Tech Stack

- **Backend**: Rust + Axum
- **Database**: SQLite (with SQLx)
- **Template Engine**: Askama
- **Authentication**: JWT + Argon2 password hashing

## Quick Start

```bash
# Clone and enter the project
cd lingerie-shop

# Copy environment config
cp .env.example .env

# Build and run
cargo run
```

The server will start at `http://localhost:3000`.

## Project Structure

```
lingerie-shop/
├── src/
│   ├── main.rs           # Entry point
│   ├── db.rs             # Database connection & migrations
│   ├── auth.rs           # JWT authentication
│   ├── models/           # Data models
│   │   ├── user.rs
│   │   ├── product.rs
│   │   ├── order.rs
│   │   └── cart.rs
│   ├── routes/           # Route handlers
│   │   ├── product.rs
│   │   ├── user.rs
│   │   ├── cart.rs
│   │   └── order.rs
│   └── templates/        # HTML templates
├── migrations/           # SQL migrations
├── static/               # Static assets (CSS, JS, images)
├── uploads/              # Uploaded product images
├── tests/                # Integration tests
├── Cargo.toml
└── .env.example
```

## API Endpoints

### Public
- `GET /` - Home page
- `GET /products` - Product listing
- `GET /products/:id` - Product detail
- `POST /auth/register` - User registration
- `POST /auth/login` - User login

### Authenticated
- `GET /cart` - View cart
- `POST /cart/add` - Add to cart
- `POST /cart/update` - Update cart item
- `POST /cart/remove` - Remove from cart
- `POST /checkout` - Create order
- `GET /orders` - Order history
- `GET /profile` - User profile

### Admin
- `GET /admin` - Dashboard
- `GET /admin/products` - Manage products
- `POST /admin/products` - Create product
- `PUT /admin/products/:id` - Update product
- `DELETE /admin/products/:id` - Delete product
- `GET /admin/orders` - Manage orders
- `PUT /admin/orders/:id/status` - Update order status

## License

MIT
