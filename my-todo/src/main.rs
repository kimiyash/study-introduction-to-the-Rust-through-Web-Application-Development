use hyper::header::CONTENT_TYPE;
use repositories::label::LabelRepository;
mod handlers;
mod repositories;
use crate::handlers::{
    label::{all_label, create_label, delete_label},
    todo::{all_todo, create_todo, delete_todo, find_todo, update_todo},
};
use crate::repositories::{
    label::LabelRepositoryForDb,
    todo::{TodoRepository, TodoRepositoryForDb},
};
use axum::{
    extract::Extension,
    routing::{delete, get, post},
    Router,
};
use dotenv::dotenv;
use sqlx::PgPool;
use std::{env, net::SocketAddr, sync::Arc};
use tower_http::cors::{Any, CorsLayer, Origin};

#[tokio::main]
async fn main() {
    // logging
    let log_level = env::var("RUST_LOG").unwrap_or("info".to_string());
    env::set_var("RUST_LOG", log_level);
    tracing_subscriber::fmt::init();
    dotenv().ok();

    let database_url = &env::var("DATABASE_URL").expect("undefined [DATABASE_URL]");

    tracing::debug!("start connect database...");
    let pool = PgPool::connect(database_url)
        .await
        .unwrap_or_else(|_| panic!("fail connect database, url is [{}]", database_url));
    let app = create_app(
        TodoRepositoryForDb::new(pool.clone()),
        LabelRepositoryForDb::new(pool.clone()),
    );
    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));

    tracing::debug!("listenting on {}", addr);

    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await
        .unwrap();
}

fn create_app<Todo: TodoRepository, Label: LabelRepository>(
    todo_repository: Todo,
    label_repository: Label,
) -> Router {
    Router::new()
        .route("/", get(root))
        .route("/todos", post(create_todo::<Todo>).get(all_todo::<Todo>))
        .route(
            "/todos/:id",
            get(find_todo::<Todo>)
                .delete(delete_todo::<Todo>)
                .patch(update_todo::<Todo>),
        )
        .route(
            "/labels",
            post(create_label::<Label>).get(all_label::<Label>),
        )
        .route("/labels/:id", delete(delete_label::<Label>))
        .layer(Extension(Arc::new(todo_repository)))
        .layer(Extension(Arc::new(label_repository)))
        .layer(
            CorsLayer::new()
                .allow_origin(Origin::exact("http://localhost:3001".parse().unwrap()))
                .allow_methods(Any)
                .allow_headers(vec![CONTENT_TYPE]),
        )
}

async fn root() -> &'static str {
    "Hello, World!"
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::repositories::{
        label::test_utils::LabelRepositoryForMemory,
        todo::{test_utils::TodoRepositoryForMemory, CreateTodo, TodoEntity},
    };
    use axum::{
        body::Body,
        http::{header, Method, Request, StatusCode},
        response::Response,
    };
    use tower::ServiceExt;

    fn build_todo_req_with_json(path: &str, method: Method, json_body: String) -> Request<Body> {
        Request::builder()
            .uri(path)
            .method(method)
            .header(header::CONTENT_TYPE, mime::APPLICATION_JSON.as_ref())
            .body(Body::from(json_body))
            .unwrap()
    }

    fn build_todo_req_with_empty(method: Method, path: &str) -> Request<Body> {
        Request::builder()
            .uri(path)
            .method(method)
            .body(Body::empty())
            .unwrap()
    }

    async fn res_to_todo(res: Response) -> TodoEntity {
        let bytes = hyper::body::to_bytes(res.into_body()).await.unwrap();
        let body: String = String::from_utf8(bytes.to_vec()).unwrap();
        let todo: TodoEntity = serde_json::from_str(&body)
            .unwrap_or_else(|_| panic!("cannot convert Todo instance. body {}", body));
        todo
    }

    #[tokio::test]
    async fn should_return_hello_world() {
        let todo_repository = TodoRepositoryForMemory::default();
        let label_repository = LabelRepositoryForMemory::default();
        let req = Request::builder().uri("/").body(Body::empty()).unwrap();
        let res = create_app(todo_repository, label_repository)
            .oneshot(req)
            .await
            .unwrap();
        let bytes = hyper::body::to_bytes(res.into_body()).await.unwrap();
        let body = String::from_utf8(bytes.to_vec()).unwrap();
        assert_eq!(body, "Hello, World!");
    }

    #[tokio::test]
    async fn should_crate_todo() {
        let expected = TodoEntity::new(1, "should_return_create_todo".to_string());

        let todo_repository = TodoRepositoryForMemory::default();
        let label_repository = LabelRepositoryForMemory::default();
        let req = build_todo_req_with_json(
            "/todos",
            Method::POST,
            r#"{ "text": "should_return_create_todo", "labels": [] }"#.to_string(),
        );
        let res = create_app(todo_repository, label_repository)
            .oneshot(req)
            .await
            .unwrap();
        let todo = res_to_todo(res).await;
        assert_eq!(expected, todo);
    }

    #[tokio::test]
    async fn should_find_todo() {
        let expected = TodoEntity::new(1, "should_find_todo".to_string());

        let labels = vec![];
        let todo_repository = TodoRepositoryForMemory::default();
        let label_repository = LabelRepositoryForMemory::default();
        todo_repository
            .create(CreateTodo::new("should_find_todo".to_string(), labels))
            .await
            .expect("faild create todo");
        let req = build_todo_req_with_empty(Method::GET, "/todos/1");
        let res = create_app(todo_repository, label_repository)
            .oneshot(req)
            .await
            .unwrap();
        let todo = res_to_todo(res).await;

        assert_eq!(expected, todo);
    }

    #[tokio::test]
    async fn should_get_all_todos() {
        let expected = TodoEntity::new(1, "should_get_all_todos".to_string());

        let labels = vec![];
        let todo_repository = TodoRepositoryForMemory::default();
        let label_repository = LabelRepositoryForMemory::default();
        todo_repository
            .create(CreateTodo::new("should_get_all_todos".to_string(), labels))
            .await
            .expect("faild create todo");
        let req = build_todo_req_with_empty(Method::GET, "/todos");
        let res = create_app(todo_repository, label_repository)
            .oneshot(req)
            .await
            .unwrap();
        let bytes = hyper::body::to_bytes(res.into_body()).await.unwrap();
        let body: String = String::from_utf8(bytes.to_vec()).unwrap();
        let todos: Vec<TodoEntity> = serde_json::from_str(&body)
            .unwrap_or_else(|_| panic!("cannot convert Todo instance. body {}", body));

        assert_eq!(vec![expected], todos);
    }

    #[tokio::test]
    async fn should_update_todo() {
        let expected = TodoEntity::new(1, "should_update_todo".to_string());

        let labels = vec![];
        let todo_repository = TodoRepositoryForMemory::default();
        let label_repository = LabelRepositoryForMemory::default();
        todo_repository
            .create(CreateTodo::new("before_update_todo".to_string(), labels))
            .await
            .expect("faild create todo");
        let req = build_todo_req_with_json(
            "/todos/1",
            Method::PATCH,
            r#"{
                "id": 1,
                "text": "should_update_todo",
                "completed": false
            }
            "#
            .to_string(),
        );
        let res = create_app(todo_repository, label_repository)
            .oneshot(req)
            .await
            .unwrap();
        let todo = res_to_todo(res).await;

        assert_eq!(expected, todo);
    }

    #[tokio::test]
    async fn should_delete_todo() {
        let labels = vec![];
        let todo_repository = TodoRepositoryForMemory::default();
        let label_repository = LabelRepositoryForMemory::default();
        todo_repository
            .create(CreateTodo::new("should_delete_todo".to_string(), labels))
            .await
            .expect("faild create todo");
        let req = build_todo_req_with_empty(Method::DELETE, "/todos/1");
        let res = create_app(todo_repository, label_repository)
            .oneshot(req)
            .await
            .unwrap();

        assert_eq!(StatusCode::NO_CONTENT, res.status());
    }
}
