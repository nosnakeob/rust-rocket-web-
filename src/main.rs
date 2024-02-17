extern crate rbatis;
#[macro_use]
extern crate rocket;

use std::sync::Arc;

use rbatis::{Error, RBatis};
use rbatis::executor::Executor;
use rbatis::intercept::{Intercept, ResultType};
use rbatis::rbdc::db::ExecResult;
use rbdc_pg::PgDriver;
use rbs::Value;
use rocket::Config;
use serde_json::json;

use auth::{check, login, register};
use domain::R;
use error::{default_catcher, not_authorized};

mod auth;
mod error;
mod domain;

#[derive(Debug)]
pub struct ReturningIdPlugin {}

#[async_trait]
impl Intercept for ReturningIdPlugin {
    async fn before(
        &self,
        _task_id: i64,
        rb: &dyn Executor,
        sql: &mut String,
        args: &mut Vec<Value>,
        result: ResultType<&mut Result<ExecResult, Error>, &mut Result<Vec<Value>, Error>>,
    ) -> Result<bool, Error> {
        if sql.contains("insert into") {
            let new_sql = format!("{} {}", sql, "returning id");

            if let ResultType::Exec(exec_r) = result {
                let id = rb.query(&new_sql, args.clone()).await?;
                let mut exec = ExecResult::default();
                exec.rows_affected = id.len() as u64;
                exec.last_insert_id = id.as_array().unwrap().last().unwrap()["id"].clone();

                *exec_r = Ok(exec);

                return Ok(false);
            }
        }

        Ok(true)
    }
}

#[get("/")]
async fn index() -> R {
    R::ok(None)
}


#[launch]
async fn rocket() -> _ {
    let rb = RBatis::new();

    let sql_addr = Config::figment().find_value("sql_addr").unwrap().as_str().unwrap().to_string();
    rb.link(PgDriver {}, sql_addr.as_str()).await.unwrap();

    rb.intercepts.insert(0, Arc::new(ReturningIdPlugin {}));

    rocket::build()
        .mount("/", routes![index,register, login, check])
        .register("/", catchers![default_catcher,not_authorized])
        .manage(rb)
}

