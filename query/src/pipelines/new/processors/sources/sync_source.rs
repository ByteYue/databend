// Copyright 2022 Datafuse Labs.
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

use common_datablocks::DataBlock;
use common_exception::ErrorCode;
use common_exception::Result;

use crate::pipelines::new::processors::port::OutputPort;
use crate::pipelines::new::processors::processor::Event;
use crate::pipelines::new::processors::processor::ProcessorPtr;
use crate::pipelines::new::processors::Processor;

/// Synchronized source. such as:
///     - Memory storage engine.
///     - SELECT * FROM numbers_mt(1000)
pub trait SyncSource: Send {
    const NAME: &'static str;

    fn generate(&mut self) -> Result<Option<DataBlock>>;
}

// TODO: This can be refactored using proc macros
pub struct SyncSourcer<T: 'static + SyncSource> {
    is_finish: bool,
    best_push_pos: usize,

    inner: T,
    outputs: Vec<Arc<OutputPort>>,
    generated_data: Option<DataBlock>,
}

impl<T: 'static + SyncSource> SyncSourcer<T> {
    pub fn create(mut outputs: Vec<Arc<OutputPort>>, inner: T) -> Result<ProcessorPtr> {
        match outputs.len() {
            0 => Err(ErrorCode::LogicalError("Source output port is empty.")),
            1 => Ok(SingleOutputSyncSourcer::create(outputs.remove(0), inner)),
            _ => Ok(ProcessorPtr::create(Box::new(Self {
                inner,
                outputs,
                is_finish: false,
                best_push_pos: 0,
                generated_data: None,
            }))),
        }
    }

    #[inline(always)]
    fn push_data_to_output(&mut self, index: usize) {
        if let Some(generated_data) = self.generated_data.take() {
            self.outputs[index].push_data(Ok(generated_data));
            self.best_push_pos += 1;
        }
    }

    #[inline(always)]
    fn push_data_to_outputs(&mut self) -> bool {
        let mut outputs_finished = true;
        let mut index = self.best_push_pos;

        loop {
            if self.outputs[index].can_push() {
                self.push_data_to_output(index);
                return false;
            }

            if outputs_finished && !self.outputs[index].is_finished() {
                outputs_finished = false;
            }

            match index >= self.outputs.len() {
                true => index = 0,
                false => index += 1,
            }

            if index == self.best_push_pos {
                return outputs_finished;
            }
        }
    }

    #[inline(always)]
    fn close_outputs(&mut self) -> Event {
        if !self.is_finish {
            self.is_finish = true;
        }

        for output_port in &self.outputs {
            if !output_port.is_finished() {
                output_port.finish();
            }
        }

        Event::Finished
    }
}

#[async_trait::async_trait]
impl<T: 'static + SyncSource> Processor for SyncSourcer<T> {
    fn name(&self) -> &'static str {
        T::NAME
    }

    fn event(&mut self) -> Result<Event> {
        Ok(match &self.generated_data {
            None if self.is_finish => self.close_outputs(),
            None => Event::Sync,
            Some(_) => match self.push_data_to_outputs() {
                true => self.close_outputs(),
                false => Event::NeedConsume,
            },
        })
    }

    fn process(&mut self) -> Result<()> {
        match self.inner.generate()? {
            None => self.is_finish = true,
            Some(data_block) => self.generated_data = Some(data_block),
        };

        Ok(())
    }
}

/// The optimization of SyncSourceProcessorWrap.
/// It's used when source has only one output port.
struct SingleOutputSyncSourcer<T: 'static + SyncSource> {
    is_finish: bool,

    inner: T,
    output: Arc<OutputPort>,
    generated_data: Option<DataBlock>,
}

impl<T: 'static + SyncSource> SingleOutputSyncSourcer<T> {
    pub fn create(output: Arc<OutputPort>, inner: T) -> ProcessorPtr {
        ProcessorPtr::create(Box::new(Self {
            inner,
            output,
            is_finish: false,
            generated_data: None,
        }))
    }

    #[inline(always)]
    fn push_data(&mut self) -> Event {
        if let Some(generated_data) = self.generated_data.take() {
            self.output.push_data(Ok(generated_data));
        }

        Event::NeedConsume
    }

    #[inline(always)]
    fn close_output(&mut self) -> Event {
        if !self.is_finish {
            self.is_finish = true;
        }

        if !self.output.is_finished() {
            self.output.finish();
        }

        Event::Finished
    }
}

#[async_trait::async_trait]
impl<T: 'static + SyncSource> Processor for SingleOutputSyncSourcer<T> {
    fn name(&self) -> &'static str {
        T::NAME
    }

    fn event(&mut self) -> Result<Event> {
        Ok(match &self.generated_data {
            None if self.is_finish => self.close_output(),
            None => Event::Sync,
            Some(_) if self.output.can_push() => self.push_data(),
            Some(_) if self.output.is_finished() => self.close_output(),
            Some(_) => Event::NeedConsume,
        })
    }

    fn process(&mut self) -> Result<()> {
        match self.inner.generate()? {
            None => self.is_finish = true,
            Some(data_block) => self.generated_data = Some(data_block),
        };

        Ok(())
    }
}
