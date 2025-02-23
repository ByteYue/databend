// Copyright 2021 Datafuse Labs.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use std::sync::Arc;

use common_exception::Result;
use common_meta_types::GrantObject;
use common_meta_types::UserPrivilegeType;
use common_planners::TruncateTablePlan;
use common_streams::DataBlockStream;
use common_streams::SendableDataBlockStream;

use crate::interpreters::Interpreter;
use crate::sessions::QueryContext;

pub struct TruncateTableInterpreter {
    ctx: Arc<QueryContext>,
    plan: TruncateTablePlan,
}

impl TruncateTableInterpreter {
    pub fn try_create(ctx: Arc<QueryContext>, plan: TruncateTablePlan) -> Result<Self> {
        Ok(TruncateTableInterpreter { ctx, plan })
    }
}

#[async_trait::async_trait]
impl Interpreter for TruncateTableInterpreter {
    fn name(&self) -> &str {
        "TruncateTableInterpreter"
    }

    async fn execute(&self) -> Result<SendableDataBlockStream> {
        let catalog_name = self.plan.catalog.as_str();
        let db_name = self.plan.database.as_str();
        let tbl_name = self.plan.table.as_str();

        self.ctx
            .get_current_session()
            .validate_privilege(
                &GrantObject::Table(catalog_name.into(), db_name.into(), tbl_name.into()),
                UserPrivilegeType::Delete,
            )
            .await?;

        let tbl = self.ctx.get_table(catalog_name, db_name, tbl_name).await?;
        tbl.truncate(self.ctx.clone(), self.plan.clone()).await?;
        Ok(Box::pin(DataBlockStream::create(
            self.plan.schema(),
            None,
            vec![],
        )))
    }
}
