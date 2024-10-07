use anyhow::Context;
use axum::async_trait;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use std::{
    collections::HashMap,
    sync::{Arc, RwLock, RwLockReadGuard, RwLockWriteGuard},
};
use thiserror::Error;
use validator::Validate;

#[derive(Debug, Error, PartialEq, Eq)]
enum RepositoryError {
    #[error("Not Found, id is {0}")]
    NotFound(i32),
}

#[async_trait]
pub trait TodoRepository: Clone + std::marker::Send + std::marker::Sync + 'static {
    async fn create(&self, payload: CreateTodo) -> anyhow::Result<Todo>;
    async fn find(&self, id: i32) -> anyhow::Result<Todo>;
    async fn all(&self) -> anyhow::Result<Vec<Todo>>;
    async fn update(&self, id: i32, payload: UpdateTodo) -> anyhow::Result<Todo>;
    async fn delete(&self, id: i32) -> anyhow::Result<()>;
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct Todo {
    id: i32,
    text: String,
    completed: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq, Validate)]
pub struct CreateTodo {
    #[validate(length(min = 1, message = "Can not be Empty"))]
    #[validate(length(max = 100, message = "Over text length"))]
    text: String,
}

#[cfg(test)]
impl CreateTodo {
    pub fn new(text: String) -> Self {
        Self { text }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq, Validate)]
pub struct UpdateTodo {
    #[validate(length(min = 1, message = "Can not be Empty"))]
    #[validate(length(max = 100, message = "Over text length"))]
    text: Option<String>,
    completed: Option<bool>,
}

impl Todo {
    pub fn new(id: i32, text: String) -> Self {
        Self {
            id,
            text,
            completed: false,
        }
    }
}

type TodoDatas = HashMap<i32, Todo>;

#[derive(Debug, Clone)]
pub struct TodoRepositoryForMemory {
    store: Arc<RwLock<TodoDatas>>,
}

impl TodoRepositoryForMemory {
    pub fn new() -> Self {
        TodoRepositoryForMemory {
            store: Arc::default(),
        }
    }

    fn write_store_ref(&self) -> RwLockWriteGuard<TodoDatas> {
        self.store.write().unwrap()
    }

    fn read_store_ref(&self) -> RwLockReadGuard<TodoDatas> {
        self.store.read().unwrap()
    }
}

impl Default for TodoRepositoryForMemory {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl TodoRepository for TodoRepositoryForMemory {
    async fn create(&self, payload: CreateTodo) -> anyhow::Result<Todo> {
        let mut store = self.write_store_ref();
        let id = (store.len() + 1) as i32;
        let todo = Todo::new(id, payload.text.clone());
        store.insert(id, todo.clone());
        Ok(todo)
    }

    async fn find(&self, id: i32) -> anyhow::Result<Todo> {
        let store = self.read_store_ref();
        let todo = store
            .get(&id)
            .cloned()
            .ok_or(RepositoryError::NotFound(id))?;
        Ok(todo)
    }

    async fn all(&self) -> anyhow::Result<Vec<Todo>> {
        Ok(Vec::from_iter(self.read_store_ref().values().cloned()))
    }

    async fn update(&self, id: i32, payload: UpdateTodo) -> anyhow::Result<Todo> {
        let mut store = self.write_store_ref();
        let mut todo = store
            .get(&id)
            .context(RepositoryError::NotFound(id))?
            .clone();
        if let Some(text) = payload.text {
            todo.text = text;
        }
        if let Some(completed) = payload.completed {
            todo.completed = completed;
        }
        store.insert(todo.id, todo.clone());
        Ok(todo)
    }

    async fn delete(&self, id: i32) -> anyhow::Result<()> {
        self.write_store_ref()
            .remove(&id)
            .context(RepositoryError::NotFound(id))?;
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct TodoRepositoryForDb {
    pool: PgPool,
}

impl TodoRepositoryForDb {
    pub fn new(pool: PgPool) -> Self {
        TodoRepositoryForDb { pool }
    }
}

#[async_trait]
impl TodoRepository for TodoRepositoryForDb {
    async fn create(&self, _payload: CreateTodo) -> anyhow::Result<Todo> {
        todo!()
    }

    async fn find(&self, _id: i32) -> anyhow::Result<Todo> {
        todo!()
    }

    async fn all(&self) -> anyhow::Result<Vec<Todo>> {
        todo!()
    }

    async fn update(&self, _id: i32, _payload: UpdateTodo) -> anyhow::Result<Todo> {
        todo!()
    }

    async fn delete(&self, _id: i32) -> anyhow::Result<()> {
        todo!()
    }
}

#[cfg(test)]
mod tests {
    use axum::async_trait;

    use crate::repositories::UpdateTodo;

    use super::{CreateTodo, Todo, TodoRepository, TodoRepositoryForMemory};

    #[tokio::test]
    async fn todo_crud_scenario() {
        let repository = TodoRepositoryForMemory::new();
        let id = 1;
        let text = "test1".to_string();
        let completed = false;

        // create
        let todo = CreateTodo { text: text.clone() };
        repository.create(todo).await.expect("failed create todo");

        // find
        let todo = repository.find(id).await.unwrap();
        assert_eq!(
            Todo {
                id,
                text: text.clone(),
                completed,
            },
            todo
        );

        // update
        let text = "test2".to_string();
        let completed = true;
        assert_eq!(
            Todo {
                id,
                text: text.clone(),
                completed,
            },
            repository
                .update(
                    id,
                    UpdateTodo {
                        text: Some(text.clone()),
                        completed: Some(completed),
                    }
                )
                .await
                .unwrap()
        );

        // all
        assert_eq!(
            [Todo {
                id,
                text: text.clone(),
                completed,
            }]
            .to_vec(),
            repository.all().await.expect("faild get all todo")
        );

        // delete
        assert!(repository.delete(id).await.is_ok());
    }
}
