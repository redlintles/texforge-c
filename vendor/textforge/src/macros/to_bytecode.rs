#[macro_export]
macro_rules! to_bytecode {
    // 1) Forma antiga: lista literal
    ($opcode:expr, [$($param:expr),* $(,)?]) => {
        {
        use $crate::context::execution_context::GlobalExecutionContext;

        // Coleta os params pra contar e iterar
        let params_vec: Vec<$crate::parser::params::TextForgeParamTypes> = vec![$($param),*];

        let opcode_u32: u32 = $opcode;
        let param_count_u8: u8 = params_vec
            .len()
            .try_into()
            .expect("Param count exceeds u8::MAX");

        // Body = [opcode u32][param_count u8][params...]
        let mut body: Vec<u8> = Vec::new();
        body.extend_from_slice(&opcode_u32.to_be_bytes());
        body.push(param_count_u8);

        let mut ctx = GlobalExecutionContext::new();

        for p in &params_vec {
            p.write_as_instruction_param(&mut body, &mut ctx)?;
        }

        // Instruction Total Size = bytes do body (4 + 1 + params...)
        let instruction_total_size_u64: u64 = body.len() as u64;

        // Final = [total_size u64] + body
        let mut out: Vec<u8> = Vec::with_capacity(8 + body.len());
        out.extend_from_slice(&instruction_total_size_u64.to_be_bytes());
        out.extend_from_slice(&body);

        out
        }
    };

    // 2) Nova forma: recebe um "container" (vec, slice, &vec, etc.)
    //    que vire uma fatia via AsRef<[TextForgeParamTypes]>
    ($opcode:expr, $params:expr) => {
        {
        use $crate::context::execution_context::GlobalExecutionContext;

        // Importante: fixa o tipo para evitar inferência ruim
        let params_slice: &[$crate::parser::params::TextForgeParamTypes] =
            ::core::convert::AsRef::<[$crate::parser::params::TextForgeParamTypes]>::as_ref(&$params);

        let opcode_u32: u32 = $opcode;
        let param_count_u8: u8 = params_slice
            .len()
            .try_into()
            .expect("Param count exceeds u8::MAX");

        // Body = [opcode u32][param_count u8][params...]
        let mut body: Vec<u8> = Vec::new();
        body.extend_from_slice(&opcode_u32.to_be_bytes());
        body.push(param_count_u8);

        let mut ctx = GlobalExecutionContext::new();

        for p in params_slice {
            p.write_as_instruction_param(&mut body, &mut ctx)?;
        }

        // Instruction Total Size = bytes do body (4 + 1 + params...)
        let instruction_total_size_u64: u64 = body.len() as u64;

        // Final = [total_size u64] + body
        let mut out: Vec<u8> = Vec::with_capacity(8 + body.len());
        out.extend_from_slice(&instruction_total_size_u64.to_be_bytes());
        out.extend_from_slice(&body);

        out
        }
    };
}
