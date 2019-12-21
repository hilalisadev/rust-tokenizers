// Copyright 2018 The Open AI Team Authors
// Copyright 2018 The HuggingFace Inc. team.
// Copyright 2019 Guillaume Becquin
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//     http://www.apache.org/licenses/LICENSE-2.0
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use crate::OpenAiGptVocab;
use crate::preprocessing::vocab::base_vocab::Vocab;
use crate::preprocessing::tokenizer::base_tokenizer::{Tokenizer, BaseTokenizer};
use std::collections::HashMap;
use crate::preprocessing::tokenizer::tokenization_utils::{split_on_special_tokens, openai_gpt_bpe};
use std::rc::Rc;
use std::cell::RefCell;
use crate::preprocessing::vocab::bpe_vocab::BpePairVocab;
use std::sync::Arc;

pub struct OpenAiGptTokenizer {
    vocab: Arc<OpenAiGptVocab>,
    base_tokenizer: BaseTokenizer<OpenAiGptVocab>,
    bpe_ranks: Rc<BpePairVocab>,
    cache: RefCell<HashMap<String, Vec<String>>>,
}

impl OpenAiGptTokenizer {
    pub fn from_file(vocab_path: &str, merges_path: &str) -> OpenAiGptTokenizer {
        let vocab = Arc::new(OpenAiGptVocab::from_file(vocab_path));
        let base_tokenizer = BaseTokenizer::from_existing_vocab(vocab.clone());
        let bpe_ranks = Rc::new(BpePairVocab::from_file(merges_path));
        let cache = RefCell::new(HashMap::new());
        OpenAiGptTokenizer { vocab, base_tokenizer, bpe_ranks, cache}
    }

    pub fn from_existing_vocab_and_merges(vocab: Arc<OpenAiGptVocab>, merges: Rc<BpePairVocab>) -> OpenAiGptTokenizer {
        let base_tokenizer = BaseTokenizer::from_existing_vocab(vocab.clone());
        let cache = RefCell::new(HashMap::new());
        OpenAiGptTokenizer { vocab, base_tokenizer, bpe_ranks: merges, cache}
    }
}

impl Tokenizer<OpenAiGptVocab> for OpenAiGptTokenizer {
    fn vocab(&self) -> &OpenAiGptVocab {
        &self.vocab
    }

    fn tokenize(&self, text: &str) -> Vec<String> {
        let mut tokenized_text: Vec<String> = Vec::with_capacity(text.len());

        let temp_text = split_on_special_tokens(text, self.vocab.as_ref());

        for text in temp_text {
            if !self.vocab.special_values.contains_key(text) {
                let sub_words: Vec<String> = self.base_tokenizer.tokenize(text);

                for word in sub_words {
                    let cached: bool = match self.cache.borrow().get(&word) {
                        Some(value) => {
                            tokenized_text.extend(value.clone());
                            true
                        }
                        None => false
                    };
                    if !cached {
                        let bpe_output = openai_gpt_bpe(&word, &self.bpe_ranks);
                        self.cache.borrow_mut().insert(word.to_owned(), bpe_output.clone());
                        tokenized_text.extend(bpe_output);
                    }
                };
            } else {
                tokenized_text.push(text.to_owned());
            }
        }
        tokenized_text
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::OpenAiGptVocab;
    use std::collections::HashMap;
    use crate::preprocessing::tokenizer::base_tokenizer::{TruncationStrategy, TokenizedInput};

    fn generate_test_vocab() -> OpenAiGptVocab {
        let values: HashMap<String, i64> = [
            ("t".to_owned(), 0),
            ("h".to_owned(), 1),
            ("a</w>".to_owned(), 2),
            ("n".to_owned(), 3),
            ("the".to_owned(), 4),
            ("Ġ".to_owned(), 5),
            ("<unk>".to_owned(), 6),
            ("o</w>".to_owned(), 7)
        ].iter().cloned().collect();

        let special_values: HashMap<String, i64> = [
            ("<unk>".to_owned(), 6),
        ].iter().cloned().collect();

        OpenAiGptVocab { values, unknown_value: "<unk>", special_values }
    }

    fn generate_test_merges() -> BpePairVocab {
        let values: HashMap<(String, String), i64> = [
            (("4".to_owned(), "t".to_owned()), 0),
            (("2".to_owned(), "n".to_owned()), 1),
            (("r".to_owned(), "th</w>".to_owned()), 2),
            (("t".to_owned(), "he</w>".to_owned()), 3),
            (("h".to_owned(), "e".to_owned()), 4),
            (("t".to_owned(), "h</w>".to_owned()), 5),
            (("t".to_owned(), "h".to_owned()), 6),
        ].iter().cloned().collect();


        BpePairVocab { values }
    }

    #[test]
    fn test_openai_gpt_tokenizer() {
//        Given
        let vocab = Arc::new(generate_test_vocab());
        let merges = Rc::new(generate_test_merges());
        let openai_gpt_tokenizer: OpenAiGptTokenizer = OpenAiGptTokenizer::from_existing_vocab_and_merges(vocab, merges);
        let test_tuples = [
            (
                "the earth",
                vec!("th", "e</w>", "e", "a", "rth</w>")
            ),
            (
                "",
                vec!()
            ),
            (
                " ",
                vec!("<unk>")
            ),
            (
                " \n ",
                vec!("<unk>")
            ),
        ];
        let source_texts: Vec<&str> = test_tuples.iter().map(|v| v.0).collect();
        let expected_results: Vec<Vec<&str>> = test_tuples.iter().map(|v| v.1.clone()).collect();

//        When & Then
        for (source_text, expected_result) in test_tuples.iter() {
            assert_eq!(openai_gpt_tokenizer.tokenize(*source_text), *expected_result);
        }

        assert_eq!(openai_gpt_tokenizer.tokenize_list(source_texts.clone()), expected_results);
    }


    #[test]
    fn test_encode() {
//        Given
        let vocab = Arc::new(generate_test_vocab());
        let merges = Rc::new(generate_test_merges());
        let openai_gpt_tokenizer: OpenAiGptTokenizer = OpenAiGptTokenizer::from_existing_vocab_and_merges(vocab, merges);
        let truncation_strategy = TruncationStrategy::LongestFirst;
        let test_tuples = [
            (
                "the earth",
                TokenizedInput { token_ids: vec!(6, 6, 6, 6, 6), segment_ids: vec!(0, 0, 0, 0, 0), special_tokens_mask: vec!(0, 0, 0, 0, 0), overflowing_tokens: vec!(), num_truncated_tokens: 0 }
            ),
            (
                " ",
                TokenizedInput { token_ids: vec!(6), segment_ids: vec!(0), special_tokens_mask: vec!(0), overflowing_tokens: vec!(), num_truncated_tokens: 0 }
            ),
            (
                "",
                TokenizedInput { token_ids: vec!(), segment_ids: vec!(), special_tokens_mask: vec!(), overflowing_tokens: vec!(), num_truncated_tokens: 0 }
            )
        ];
        let source_texts: Vec<&str> = test_tuples.iter().map(|v| v.0).collect();
        let expected_results: Vec<TokenizedInput> = test_tuples.iter().map(|v| v.1.clone()).collect();

//        When & Then
        for (source_text, expected_result) in test_tuples.iter() {
            assert_eq!(openai_gpt_tokenizer.encode(source_text, None, 128, &truncation_strategy, 0),
                       *expected_result);
        }
        assert_eq!(openai_gpt_tokenizer.encode_list(source_texts.clone(), 128, &truncation_strategy, 0), expected_results);
    }
}