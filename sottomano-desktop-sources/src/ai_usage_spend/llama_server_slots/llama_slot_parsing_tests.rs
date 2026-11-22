use super::{
    llama_server_listings::{ListedSlot, ModelList},
    *,
};

#[test]
fn listed_models_carry_status_and_child_instance() {
    let listed: ModelList = serde_json::from_str(
        r#"{"data":[
            {"id":"Qwen3.6","status":{"value":"unloaded","args":["llama-server","--host","127.0.0.1","--port","0"]}},
            {"id":"Qwen3.8","status":{"value":"loaded","args":["llama-server","--host","127.0.0.1","--port","57883","--ctx-size","32768","--model","/models/Qwen3.8-27B.gguf"]}}
        ]}"#,
    )
    .unwrap();
    let models: Vec<LlamaModel> = listed.data.into_iter().map(LlamaModel::from).collect();
    assert_eq!(models[0].status, LlamaModelStatus::Unloaded);
    assert_eq!(models[0].instance, ChildInstance::NotListening);
    assert_eq!(models[0].model_file, ModelFileArgument::NotGiven);
    assert_eq!(
        models[1].model_file,
        ModelFileArgument::Given(PathBuf::from("/models/Qwen3.8-27B.gguf"))
    );
    assert_eq!(
        models[1].instance,
        ChildInstance::Listening(ModelInstance {
            host: "127.0.0.1".to_owned(),
            port: 57883,
        })
    );
}

#[test]
fn slots_without_a_task_have_no_token_counts() {
    let listed: Vec<ListedSlot> = serde_json::from_str(
        r#"[
            {"id":0,"n_ctx":32768,"speculative":false,"is_processing":false},
            {"id":3,"n_ctx":32768,"is_processing":true,"id_task":182,"n_prompt_tokens":11745,
             "next_token":[{"has_next_token":true,"n_remain":-1,"n_decoded":129}]}
        ]"#,
    )
    .unwrap();
    let slots: Vec<LlamaSlot> = listed.into_iter().map(LlamaSlot::from).collect();
    assert_eq!(slots[0].task, SlotAssignment::Unassigned);
    let SlotAssignment::Assigned(task) = slots[1].task else {
        panic!("a processing slot carries its task");
    };
    assert_eq!(task.tokens(), 11874);
}
