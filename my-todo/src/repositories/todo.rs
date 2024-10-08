use axum::async_trait;
use indoc::indoc;
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, PgPool};
use validator::Validate;
use crate::repositories::label::Label;

use super::RepositoryError;

#[async_trait]
pub trait TodoRepository: Clone + std::marker::Send + std::marker::Sync + 'static {
    async fn create(&self, payload: CreateTodo) -> anyhow::Result<TodoEntity>;
    async fn find(&self, id: i32) -> anyhow::Result<TodoEntity>;
    async fn all(&self) -> anyhow::Result<Vec<TodoEntity>>;
    async fn update(&self, id: i32, payload: UpdateTodo) -> anyhow::Result<TodoEntity>;
    async fn delete(&self, id: i32) -> anyhow::Result<()>;
}

#[derive(Debug, Clone, PartialEq, Eq, FromRow)]
pub struct TodoWithLabelFromRow {
    id: i32,
    text: String,
    completed: bool,
    // label_id, Option<i32>
    // label_name, Option<String>
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct TodoEntity {
    pub id: i32,
    pub text: String,
    pub completed: bool,
    pub labels: Vec<Label>,
}

fn fold_entities(row: Vec<TodoWithLabelFromRow>) -> Vec<TodoEntity> {
    row.iter()
        .fold(vec![], |mut accm: Vec<TodoEntity>, current| {
            // todo 同一id の todo を畳み込み
            // todo 同一id の場合、Labelを作成し `labels` へ push
            accm.push(TodoEntity {
                id: current.id,
                text: current.text.clone(),
                completed: current.completed,
                labels: vec![]
            });
            accm
        })
}

fn fold_entity(row: TodoWithLabelFromRow) -> TodoEntity {
    let todo_entities = fold_entities(vec![row]);
    let todo = todo_entities.first().expect("expect 1 todo");

    todo.clone()
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq, Validate)]
pub struct CreateTodo {
    #[validate(length(min = 1, message = "Can not be Empty"))]
    #[validate(length(max = 100, message = "Over text length"))]
    text: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq, Validate)]
pub struct UpdateTodo {
    #[validate(length(min = 1, message = "Can not be Empty"))]
    #[validate(length(max = 100, message = "Over text length"))]
    text: Option<String>,
    completed: Option<bool>,
}

#[cfg(test)]
impl TodoEntity {
    pub fn new(id: i32, text: String) -> Self {
        Self {
            id,
            text,
            completed: false,
            labels: vec![],
        }
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
    async fn create(&self, payload: CreateTodo) -> anyhow::Result<TodoEntity> {
        let todo = sqlx::query_as::<_, TodoWithLabelFromRow>(indoc!(
            r#"
                insert into todos (text, completed) values ($1, false)
                returning *
            "#,
        ))
        .bind(payload.text.clone())
        .fetch_one(&self.pool)
        .await?;

        Ok(fold_entity(todo))
    }

    async fn find(&self, id: i32) -> anyhow::Result<TodoEntity> {
        let todo = sqlx::query_as::<_, TodoWithLabelFromRow>(indoc!(
            r#"
                select * from todos where id = $1
            "#,
        ))
        .bind(id)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| match e {
            sqlx::Error::RowNotFound => RepositoryError::NotFound(id),
            _ => RepositoryError::Unexpected(e.to_string()),
        })?;

        Ok(fold_entity(todo))
    }

    async fn all(&self) -> anyhow::Result<Vec<TodoEntity>> {
        let todos = sqlx::query_as::<_, TodoWithLabelFromRow>(indoc!(
            r#"
                select * from todos order by id desc
            "#,
        ))
        .fetch_all(&self.pool)
        .await?;

        Ok(fold_entities(todos))
    }

    async fn update(&self, id: i32, payload: UpdateTodo) -> anyhow::Result<TodoEntity> {
        let old_todo = self.find(id).await?;
        let todo = sqlx::query_as::<_, TodoWithLabelFromRow>(indoc!(
            r#"
                update todos set text = $1, completed = $2 where id = $3
                returning *
            "#,
        ))
        .bind(payload.text.unwrap_or(old_todo.text))
        .bind(payload.completed.unwrap_or(old_todo.completed))
        .bind(id)
        .fetch_one(&self.pool)
        .await?;

        Ok(fold_entity(todo))
    }

    async fn delete(&self, id: i32) -> anyhow::Result<()> {
        sqlx::query(indoc!(
            r#"
                delete from todos where id = $1
            "#
        ))
        .bind(id)
        .execute(&self.pool)
        .await
        .map_err(|e| match e {
            sqlx::Error::RowNotFound => RepositoryError::NotFound(id),
            _ => RepositoryError::Unexpected(e.to_string()),
        })?;

        Ok(())
    }
}

#[cfg(test)]
#[cfg(feature = "database-test")]
mod test {
    use super::*;
    use dotenv::dotenv;
    use sqlx::PgPool;
    use std::env;

    #[tokio::test]
    async fn crud_scenario() {
        dotenv().ok();
        let database_url = &env::var("DATABASE_URL").expect("undefined [DATABASE_URL");
        let pool = PgPool::connect(database_url)
            .await
            .unwrap_or_else(|_| panic!("fail connect database, url is [{}]", database_url));

        let repository = TodoRepositoryForDb::new(pool.clone());
        let todo_text = "[crud_scenario] text";

        // create
        let created = repository
            .create(CreateTodo::new(todo_text.to_string()))
            .await
            .expect("[create] returned Err");
        assert_eq!(created.text, todo_text);
        assert!(!created.completed);

        // find
        let todo = repository
            .find(created.id)
            .await
            .expect("[find] returned Err");
        assert_eq!(created, todo);

        // all
        let todos = repository.all().await.expect("[all] returned Err]");
        let todo = todos.first().unwrap();
        assert_eq!(created, *todo);

        // update
        let updated_text = "[crud_scenario] updated text";
        let todo = repository
            .update(
                todo.id,
                UpdateTodo {
                    text: Some(updated_text.to_string()),
                    completed: Some(true),
                },
            )
            .await
            .expect("[update] returned Err");
        assert_eq!(created.id, todo.id);
        assert_eq!(todo.text, updated_text);
        assert!(todo.completed);

        // delete
        repository
            .delete(todo.id)
            .await
            .expect("[delete] returned Err");
        let res = repository.find(created.id).await;
        assert!(res.is_err());

        let todo_rows = sqlx::query(indoc!(
            r#"
                select * from todos where id = $1
            "#
        ))
        .bind(todo.id)
        .fetch_all(&pool)
        .await
        .expect("[delete] todo_labels fetch error");
        assert!(todo_rows.is_empty());
    }
}

#[cfg(test)]
pub mod test_utils {
    use super::*;
    use anyhow::Context;
    use std::{
        collections::HashMap,
        sync::{Arc, RwLock, RwLockReadGuard, RwLockWriteGuard},
    };

    impl CreateTodo {
        pub fn new(text: String) -> Self {
            Self { text }
        }
    }

    type TodoDatas = HashMap<i32, TodoEntity>;

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
        async fn create(&self, payload: CreateTodo) -> anyhow::Result<TodoEntity> {
            let mut store = self.write_store_ref();
            let id = (store.len() + 1) as i32;
            let todo = TodoEntity::new(id, payload.text.clone());
            store.insert(id, todo.clone());
            Ok(todo)
        }

        async fn find(&self, id: i32) -> anyhow::Result<TodoEntity> {
            let store = self.read_store_ref();
            let todo = store
                .get(&id)
                .cloned()
                .ok_or(RepositoryError::NotFound(id))?; 
            Ok(todo)
        }

        async fn all(&self) -> anyhow::Result<Vec<TodoEntity>> {
            Ok(Vec::from_iter(self.read_store_ref().values().cloned()))
        }

        async fn update(&self, id: i32, payload: UpdateTodo) -> anyhow::Result<TodoEntity> {
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

    mod test {
        use super::{CreateTodo, TodoEntity, TodoRepository, TodoRepositoryForMemory};
        use crate::repositories::todo::UpdateTodo;

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
                TodoEntity {
                    id,
                    text: text.clone(),
                    completed,
                    labels: vec![],
                },
                todo
            );

            // update
            let text = "test2".to_string();
            let completed = true;
            assert_eq!(
                TodoEntity {
                    id,
                    text: text.clone(),
                    completed,
                    labels: vec![],
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
                [TodoEntity {
                    id,
                    text: text.clone(),
                    completed,
                    labels: vec![],
                }]
                .to_vec(),
                repository.all().await.expect("faild get all todo")
            );

            // delete
            assert!(repository.delete(id).await.is_ok());
        }
    }
}
