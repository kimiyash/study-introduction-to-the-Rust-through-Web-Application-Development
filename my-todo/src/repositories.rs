use anyhow::Context;
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    sync::{Arc, RwLock, RwLockReadGuard, RwLockWriteGuard},
};
use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
enum RepositoryError {
    #[error("Not Found, id is {0}")]
    NotFound(i32),
}

pub trait TodoRepository: Clone + std::marker::Send + std::marker::Sync + 'static {
    fn create(&self, payload: CreateTodo) -> Todo;
    fn find(&self, id: i32) -> Option<Todo>;
    fn all(&self) -> Vec<Todo>;
    fn update(&self, id: i32, payload: UpdateTodo) -> anyhow::Result<Todo>;
    fn delete(&self, id: i32) -> anyhow::Result<()>;
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct Todo {
    id: i32,
    text: String,
    completed: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct CreateTodo {
    text: String,
}

#[cfg(test)]
impl CreateTodo {
    pub fn new(text: String) -> Self {
        Self { text }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct UpdateTodo {
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

impl TodoRepository for TodoRepositoryForMemory {
    fn create(&self, payload: CreateTodo) -> Todo {
        let mut store = self.write_store_ref();
        let id = (store.len() + 1) as i32;
        let todo = Todo::new(id, payload.text.clone());
        store.insert(id, todo.clone());
        todo
    }

    fn find(&self, id: i32) -> Option<Todo> {
        self.read_store_ref().get(&id).cloned()
    }

    fn all(&self) -> Vec<Todo> {
        Vec::from_iter(self.read_store_ref().values().cloned())
    }

    fn update(&self, id: i32, payload: UpdateTodo) -> anyhow::Result<Todo> {
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

    fn delete(&self, id: i32) -> anyhow::Result<()> {
        self.write_store_ref()
            .remove(&id)
            .context(RepositoryError::NotFound(id))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::repositories::UpdateTodo;

    use super::{CreateTodo, Todo, TodoRepository, TodoRepositoryForMemory};

    #[test]
    fn todo_crud_scenario() {
        let repository = TodoRepositoryForMemory::new();
        let id = 1;
        let text = "test1".to_string();
        let completed = false;

        // create
        let todo = CreateTodo { text: text.clone() };
        repository.create(todo);

        // find
        let todo = repository.find(id).unwrap();
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
            repository.all()
        );

        // delete
        assert!(repository.delete(id).is_ok());
    }
}
