        let (group_name, script) = TEST_GROUPS[idx];
        println!("[initproc] select bit{} group={}", idx, group_name);
        if idx == 3 {
            // libctest：逐条运行并跳过卡死/崩溃测例，避免某条 hang 把整组带走
            run_libctest_group(environ, "/musl\0");
            run_libctest_group(environ, "/glibc\0");
        } else if group_name == "ltp" {
            // LTP: only run MUSL with filtered test cases (glibc LTP has 0% success rate)
            run_ltp_filtered(environ);
        } else {
            run_group_in_dir(environ, "/musl\0", script);
            run_group_in_dir(environ, "/glibc\0", script);
        }
    println!("[initproc] run_selected_groups done");
}