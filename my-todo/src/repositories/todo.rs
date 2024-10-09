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
    label_id: Option<i32>,
    label_name: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct TodoEntity {
    pub id: i32,
    pub text: String,
    pub completed: bool,
    pub labels: Vec<Label>,
}

#[derive(Debug, Clone, PartialEq, Eq, FromRow)]
pub struct TodoFromRow {
    id: i32,
    text: String,
    completed: bool,
}

fn fold_entities(rows: Vec<TodoWithLabelFromRow>) -> Vec<TodoEntity> {
    let rows = rows.iter();
    let mut accm: Vec<TodoEntity> = vec![];
    'outer: for row in rows {
        let todos = accm.iter_mut();
        for todo in todos {
            // id が一致 = Todo に紐づくラベルが複数存在している
            if todo.id == row.id {
                todo.labels.push(Label {
                    id: row.label_id.unwrap(),
                    name: row.label_name.clone().unwrap(),
                });
                continue 'outer;
            }
        }

        // Todo の id に一致がなかったときのみ到着、TodoEntity を作成
        let labels = if row.label_id.is_some() {
            vec![Label {
                id: row.label_id.unwrap(),
                name: row.label_name.clone().unwrap()
            }]
        } else {
            vec![]
        };

        accm.push(TodoEntity {
            id: row.id,
            text: row.text.clone(),
            completed: row.completed,
            labels,
        });
    }

    accm
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq, Validate)]
pub struct CreateTodo {
    #[validate(length(min = 1, message = "Can not be Empty"))]
    #[validate(length(max = 100, message = "Over text length"))]
    text: String,
    labels: Vec<i32>,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq, Validate)]
pub struct UpdateTodo {
    #[validate(length(min = 1, message = "Can not be Empty"))]
    #[validate(length(max = 100, message = "Over text length"))]
    text: Option<String>,
    completed: Option<bool>,
    labels: Option<Vec<i32>>,
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
        let tx = self.pool.begin().await?;

        let row = sqlx::query_as::<_, TodoFromRow>(indoc!(
            r#"
                insert into todos (text, completed) values ($1, false)
                returning *
            "#,
        ))
        .bind(payload.text.clone())
        .fetch_one(&self.pool)
        .await?;

        sqlx::query(indoc! {
            r#"
                insert into todo_labels (todo_id, label_id)
                    select $1, id from unnest($2) as t(id)
            "#
        })
        .bind(row.id)
        .bind(payload.labels)
        .execute(&self.pool)
        .await?;

        tx.commit().await?;

        let todo = self.find(row.id).await?;        
        Ok(todo)
    }

    async fn find(&self, id: i32) -> anyhow::Result<TodoEntity> {
        let items = sqlx::query_as::<_, TodoWithLabelFromRow>(indoc!(
            r#"
                select todos.*, labels.id as label_id, labels.name as label_name
                    from todos
                        left outer join todo_labels t1 on todos.id = t1.todo_id
                        left outer join labels on labels.id = t1.label_id
                    where todos.id = $1
            "#,
        ))
        .bind(id)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| match e {
            sqlx::Error::RowNotFound => RepositoryError::NotFound(id),
            _ => RepositoryError::Unexpected(e.to_string()),
        })?;

        let todos = fold_entities(items);
        let todo = todos.first().ok_or(RepositoryError::NotFound(id))?;
        Ok(todo.clone())
    }

    async fn all(&self) -> anyhow::Result<Vec<TodoEntity>> {
        let items = sqlx::query_as::<_, TodoWithLabelFromRow>(indoc!(
                r#"
                    select todos.*, labels.id as label_id, labels.name as label_name
                        from todos
                            left outer join todo_labels t1 on todos.id = t1.todo_id
                            left outer join labels on labels.id = t1.label_id
                        order by todos.id desc
                "#
            ))
            .fetch_all(&self.pool)
            .await?;

        Ok(fold_entities(items))
    }

    async fn update(&self, id: i32, payload: UpdateTodo) -> anyhow::Result<TodoEntity> {
        // todo update
        let tx = self.pool.begin().await?;

        let old_todo = self.find(id).await?;
        sqlx::query(indoc!(
            r#"
                update todos set text = $1, completed = $2 where id = $3
            "#
        ))
        .bind(payload.text.unwrap_or(old_todo.text))
        .bind(payload.completed.unwrap_or(old_todo.completed))
        .bind(id)
        .execute(&self.pool)
        .await?;

        if let Some(labels) = payload.labels {
            // todo's label update
            // 一度関連するレコードを削除
            sqlx::query(indoc!(
                r#"
                    delete from todo_labels where todo_id = $1
                "#
            ))
            .bind(id)
            .execute(&self.pool)
            .await?;

            sqlx::query(indoc!(
                r#"
                    insert into todo_labels (todo_id, label_id)
                        select $1, id from unnest ($2) as t(id)
                "#
            ))
            .bind(id)
            .bind(labels)
            .execute(&self.pool)
            .await?;
        };

        tx.commit().await?;

        let todo = self.find(id).await?;
        Ok(todo)
    }

    async fn delete(&self, id: i32) -> anyhow::Result<()> {
        let tx = self.pool.begin().await?;

        // todo's label delete
        sqlx::query(indoc!(    
            r#"
                delete from todo_labels where todo_id = $1
            "#
        ))
        .bind(id)
        .execute(&self.pool)
        .await
        .map_err(|e| match e {
            sqlx::Error::RowNotFound => RepositoryError::NotFound(id),
            _ => RepositoryError::Unexpected(e.to_string())
        })?;

        // todo delete
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

        tx.commit().await?;

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

    #[test]
    fn fold_entties_test() {
        let label_1 = Label {
            id: 1,
            name: String::from("label 1"),
        };
        let label_2 = Label {
            id: 2,
            name: String::from("label 2"),
        };

        let rows = vec![
            TodoWithLabelFromRow {
                id: 1,
                text: String::from("todo 1"),
                completed: false,
                label_id: Some(label_1.id),
                label_name: Some(label_1.name.clone()),
            },
            TodoWithLabelFromRow {
                id: 1,
                text: String::from("todo 1"),
                completed: false,
                label_id: Some(label_2.id),
                label_name: Some(label_2.name.clone()),
            },
            TodoWithLabelFromRow {
                id: 2,
                text: String::from("todo 2"),
                completed: false,
                label_id: Some(label_1.id),
                label_name: Some(label_1.name.clone()),
            },
        ];
        let res = fold_entities(rows);
        assert_eq!(vec![
                TodoEntity {
                    id: 1,
                    text: String::from("todo 1"),
                    completed: false,
                    labels: vec![
                        label_1.clone(),
                        label_2.clone(),
                    ]
                },
                TodoEntity {
                    id: 2,
                    text: String::from("todo 2"),
                    completed: false,
                    labels: vec![
                        label_1.clone(),
                    ],
                },
            ] as Vec<TodoEntity>,
            res
        );
    }

    #[tokio::test]
    async fn crud_scenario() {
        dotenv().ok();
        let database_url = &env::var("DATABASE_URL").expect("undefined [DATABASE_URL");
        let pool = PgPool::connect(database_url)
            .await
            .unwrap_or_else(|_| panic!("fail connect database, url is [{}]", database_url));

        let repository = TodoRepositoryForDb::new(pool.clone());
        let todo_text = "[crud_scenario] text";

        // label data prepare
        let label_name = String::from("test label");
        let option_label = sqlx::query_as::<_, Label>(indoc!(
            r#"
                select * from labels where name = $1
            "#
        ))
        .bind(label_name.clone())
        .fetch_optional(&pool)
        .await
        .expect("Faild to prepare label data.");

        let label_1 = if let Some(label) = option_label {
            label
        } else {
            sqlx::query_as::<_, Label>(indoc!(
                r#"
                    insert into labels ( name ) values ( $1 )
                    returning *
                "#
            ))
            .bind(label_name.clone())
            .fetch_one(&pool)
            .await
            .expect("Faild insert label data.")
        };

        // create
        let created = repository
            .create(CreateTodo::new(todo_text.to_string(), vec![label_1.id]))
            .await
            .expect("[create] returned Err");
        assert_eq!(created.text, todo_text);
        assert!(!created.completed);
        assert_eq!(*created.labels.first().unwrap(), label_1);

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
                    labels: Some(vec![]),
                },
            )
            .await
            .expect("[update] returned Err");
        assert_eq!(created.id, todo.id);
        assert_eq!(todo.text, updated_text);
        assert!(todo.completed);
        assert!(todo.labels.is_empty());

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

        let rows = sqlx::query(indoc!(
            r#"
                select * from todo_labels where todo_id = $1
            "#
        ))
        .bind(todo.id)
        .fetch_all(&pool)
        .await
        .expect("[delete] todo_labels fetch error");
        assert!(rows.is_empty());

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
        pub fn new(text: String, labels: Vec<i32>) -> Self {
            Self { text, labels }
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
            let labels = vec![];
            let todo = CreateTodo { text: text.clone(), labels };
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
                            labels: Some(vec![]),
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
