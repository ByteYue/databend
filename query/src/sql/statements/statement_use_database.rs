// Copyright 2020 Datafuse Labs.
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

use common_exception::ErrorCode;
use common_exception::Result;
use common_planners::PlanNode;
use common_planners::UseDatabasePlan;
use common_tracing::tracing;
use sqlparser::ast::ObjectName;

use crate::sessions::DatabendQueryContextRef;
use crate::sql::statements::AnalyzableStatement;
use crate::sql::statements::AnalyzedResult;

#[derive(Debug, Clone, PartialEq)]
pub struct DfUseDatabase {
    pub name: ObjectName,
}

#[async_trait::async_trait]
impl AnalyzableStatement for DfUseDatabase {
    #[tracing::instrument(level = "info", skip(self, _ctx), fields(ctx.id = _ctx.get_id().as_str()))]
    async fn analyze(&self, _ctx: DatabendQueryContextRef) -> Result<AnalyzedResult> {
        if self.name.0.is_empty() {
            return Result::Err(ErrorCode::SyntaxException("Use database name is empty"));
        }

        let db = self.name.0[0].value.clone();
        Ok(AnalyzedResult::SimpleQuery(PlanNode::UseDatabase(
            UseDatabasePlan { db },
        )))
    }
}