initSidebarItems({"constant":[["USER_STACK_PAGE",""]],"enum":[["ProcessState",""]],"fn":[["exec","exec syscall"],["exit","exit syscall"],["find_available_pid",""],["fork",""],["forkret",""],["init_code","binary code of user/src/initcode.S This file will be compiled to elf, and then be stripped with objdump, as specified in Makefile."],["init_proc","Put init process into `PROCS_POOL`"],["map_stack","map user stack in `pgtable` at `stack_begin` and returns `sp`"],["sleep","put this process into sleep state"],["wakeup","wakeup process on channel"]],"static":[["PROCS_POOL_SLEEP","A Mutex that will be locked if a process is being slept but not yet put back into `PROCS_POOL`."]],"struct":[["Process",""]]});