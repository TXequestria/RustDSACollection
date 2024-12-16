//这个module没啥代码，主要是测试
//简单来说，rust的panic不意味着程序会立刻终结，而是类似其他语言抛出exception一样
//在panic发生的时候，所有作用域内的变量的drop函数都会被调用
//rust的panic可以被catch，在多线程环境下，其他线程panic，也不会导致主线程挂掉
//总之panic发生以后，程序还是可以活下来的
#[cfg(test)]
mod tests {
    use std::panic::catch_unwind;
    use std::thread;
    //panic可以被catch_undind函数捕捉,如果发生panic，会返回err
    //这不是一个很好的错误处理方式，rust也没有exception
    //正常情况不要这么干，老老实实用Option/Result
    //注意panic发生后，变量会以创建时相反的顺序被drop，与函数返回时一致
    #[test]
    fn catch_panic() {
        let result = catch_unwind(|| {
            let _a = TestDrop::new(1);
            let _b = TestDrop::new(2);
            let _c = TestDrop::new(3);
            panic!("程序从这开始爆炸了！")
        });
        assert!(result.is_err());
        println!("panic被catch了, 我没死");
    }
    //子线程panic，只会返回一个Err给主线程，而不会导致主线程一起死
    #[test]
    fn test_thread_panic() {
        let h = thread::spawn(|| {
            panic!("子线程要爆炸了！");
        });
        let return_result = h.join();
        assert!(return_result.is_err());
        println!("主线程没死");
    }
    //然而，如果panic发生时，在析构函数里又发生一次panic
    //此时程序会直接abort，不能抓住
    //你可以理解为rust不允许在panic发生时再次panic
    //也就是你不能在catch {} 里抛出异常
    //下面两个测试会失败
    #[test]
    #[should_panic]
    fn test_catch_double_panic() {
        let _ = catch_unwind(|| {
            let _a = DoublePanic{};
            panic!("这回你跑不掉了");
        });
        //走不到这里
        println!("活下来了....吗？");
    }
    #[test]
    #[should_panic]
    fn test_thread_double_panic() {
        let h = thread::spawn(|| {
            let _a = DoublePanic{};
            panic!("这回你跑不掉了");
        });
        let return_result = h.join();
        assert!(return_result.is_err());
        //走不到这里
        println!("主线程活下来了....吗？");
    }
    struct DoublePanic {}

    impl Drop for DoublePanic {
        fn drop(&mut self) {
            panic!("这回你逃不掉了😈")
        }
    }

    struct TestDrop {
        value:i32
    }

    impl TestDrop {
        fn new(value:i32) -> Self {
            Self {value}
        }
    }

    impl Drop for TestDrop {
        fn drop(&mut self) {
            println!("{}的drop正被调用",self.value);
        }
    }
}