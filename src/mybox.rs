use std::{mem::forget, ptr::{self, drop_in_place, write, read, NonNull}};
use std::ops::{Deref,DerefMut};
use std::marker::{Sync,Send};
use std::alloc::{GlobalAlloc,Layout,alloc,dealloc};
struct CAlloctor {}

//内存分配器，rust就是通过实现了GolbalAllocator trait的内存分配器类型来管理内存的
//内存分配器需要实现alloc(相当于malloc)和dealloc(相当于free)
unsafe impl GlobalAlloc for CAlloctor {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        //大小为0的时候不进行内存分配，就返回一个悬空指针
        //rust不会解引用悬空指针的
        if layout.size() == 0 {
            return NonNull::dangling().as_ptr();
        }
        //否则走malloc分配
        //我们使用libc crate直接调用C语言API，要想使用libc,就在命令行里敲"cargo add libc"就行了
        libc::malloc(layout.size()) as *mut u8
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        //大小为0的类型指针没有进行任何分配，所以什么也不干
        //free规定对空指针什么也不做，我们也一样
        if layout.size() == 0 || ptr.is_null() {
            return;
        }
        libc::free(ptr as *mut libc::c_void);
    }
}

//这个static变量就是我们的分配器了
//#[global_allocator]属性宏会自动把它设置为全局的唯一默认分配器
//之后调用alloc/dealloc就会自动指向这个变量调用malloc和free
#[global_allocator]
static CALLOCATOR:CAlloctor = CAlloctor {};

//这是一个很简单的Box,目标是教会大家Drop，Sync，Send，还有如何用FFI调用C语言
#[repr(transparent)]
pub struct MyBox<T> {
    //NonNull是非空指针
    //MyBox的安全性，就在于它绝对不能把这个ptr暴露给别人！只有它自己能持有这个指针
    //只要没有别人知道这个指针，MyBox就能保证对堆上变量的独占，因此是安全的
    ptr:NonNull<T>
}

impl<T> MyBox<T> {
    pub fn into_inner(self) -> T {
        let ptr = self.ptr;
        forget(self);
        let data = unsafe {read(ptr.as_ptr())};
        unsafe {dealloc(ptr.as_ptr() as *mut u8, Layout::new::<T>());}
        data
    }
    pub fn new(data:T) -> Self {
        let ptr = unsafe {
            //用我们刚创建的内存分配器来分配内存
            //要传入data的大小信息
            //由于data的类型是T，我们使用泛型T的大小信息就行
            alloc(Layout::new::<T>())
            //malloc 返回的是void*指针，在rust里是*mut u8，要强转一下
        } as *mut T;
        //NonNull::new()返回 Option,帮我们判空了
        let ptr = NonNull::new(ptr)
            .expect("Rust不处理内存分配失败, 失败直接panic");
        //上面之所以会用expect，是因为rust认为内存分配失败是非常严重的错误，不可恢复
        //C++ 的new 如果失败了也会直接抛异常
        unsafe {
            //把栈上的data变量写入堆内存
            write(ptr.as_ptr(),data);
        }
        Self {ptr}
    }
    //Box::into_raw的改版，就是不释放内存，返回一个裸指针
    //into_raw方法必须消耗掉MyBox自身，不然mybox的析构函数Drop被调用以后，你就得到了一个指向被释放内存区域的野指针....
    pub fn into_raw(self) -> NonNull<T> {
        let ptr = self.ptr;
        //forget以后不会调用Drop，确保返回的指针会指向有效的，未被释放的堆内存
        forget(self);
        ptr
    }
    //从一个裸指针构建MyBox是不安全的，因为裸指针本来就不安全...
    pub unsafe fn from_raw(ptr:NonNull<T>) -> Self {
        Self {ptr}
    }
}
//Drop Trait,用完Box自动释放内存
//由于MyBox实现了DropTrait，因此它是Move(移动的)
// let box2 = box1 会使得box1失效，保证指向堆内存的指针是唯一的
impl<T> Drop for MyBox<T> {
    fn drop(&mut self) {
        let ptr = self.ptr.as_ptr();
        unsafe {
            //这个drop in place很重要，因为它会负责调用堆上那个变量的析构函数
            //可以试着注释掉下面这一行，然后运行test_drop,你将不会看到析构函数
            drop_in_place(ptr);
            //用内存分配器的dealloc函数来释放内存
            //背后调用的其实是libc::free
            dealloc(ptr as *mut u8, Layout::new::<T>());
        }
    }
}

impl<T:Clone> Clone for MyBox<T> {
    fn clone(&self) -> Self {
        Self::new((**self).clone())
    }
}

//Deref trait,让我们可以通过Box来操作堆上的变量
//即我们可以通过&Box<T> 得到一个&T来访问堆上的变量
//对应解引用操作符"*"
impl<T> Deref for MyBox<T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        let reference = unsafe {self.ptr.as_ref()};
        reference
    }
}
//相同，只是可变引用
impl<T> DerefMut for MyBox<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        let mut_ref = unsafe {self.ptr.as_mut()};
        mut_ref
    }
}

//MyBox使用了裸指针，因此默认是线程不安全的
//注释掉下面两行，则test里的test_send_sync将不会编译通过
//但我们知道，MyBox<T>唯一持有一个堆上的变量，所以理论上，MyBox<T>的表现应该和T完全相同
unsafe impl<T> Send for MyBox<T> where T:Send {}
unsafe impl<T> Sync for MyBox<T> where T:Sync {}
//题外话，RC，RefCell，Cell等线程不安全的类型，本质上也只是因为他们使用了裸指针
#[cfg(test)]
mod tests {
    use std::{ptr::NonNull, rc::Rc};

    use super::MyBox;
    struct TestDrop {
    }
    impl Drop for TestDrop {
        fn drop(&mut self) {
            println!("正在Drop 0号!")
        }
    }
    #[test]
    fn test_drop() {
        let a = TestDrop {};
        MyBox::new(a);
        //如果注释掉Drop trait里 "drop_in_place(ptr);"的一行，你将不会看到任何输出
        //没注释掉的话，你应该能看到一行"正在Drop 0号", 标志着TestDrop的析构函数被正确调用
    }
    #[test]
    fn test_leak() {
        let a = TestDrop {};
        let mybox = MyBox::new(a);
        let _raw_ptr = mybox.into_raw();
    }
    #[test]
    fn test_from_raw() {
        let a = TestDrop {};
        let mybox = MyBox::new(a);
        let raw_ptr = mybox.into_raw();
        let _ = unsafe { MyBox::from_raw(raw_ptr)};
    }
    #[test]
    fn test_deref() {
        let string = String::from("123456");
        let mut string_box = MyBox::new(string);
        println!("{}",*string_box);
        *string_box = String::from("678910");
        println!("{}",*string_box);
    }
    #[test]
    fn test_send_sync() {
        //注释掉unsafe impl Sync/Send的两行，下面不会编译通过
        fn test_sync<T:Sync>(_data:&T) {}
        fn test_send<T:Sync>(_data:&T) {}
        let b = MyBox::new(0);
        test_send(&b);
        test_sync(&b)
    }
    #[test]
    fn test_sync_send_fail() {
        #[allow(dead_code)]
        fn test_sync<T:Sync>(_data:&T) {}
        #[allow(dead_code)]
        fn test_send<T:Sync>(_data:&T) {}
        let _b = MyBox::new(Rc::new(0));

        //这两行应该不能编译通过
        // test_send(&_b);
        // test_sync(&_b)
    }
    #[test]
    fn test_nonnull() {
        //对于固定大小的T，即T不是 数组, str, dyn Trait
        //Option<NonNull<T>> 和 usize以及 [u8;sizeof::<usize>()] 一样
        // 0 = None, 其他数值 = 地址为数值的Nonnull指针
        //也就是说，其实malloc返回的c_void指针是可以直接 强转为 Option<NonNull<()>>的
        use rand::Rng;
        let mut rng = rand::thread_rng();
        let none_bytes = [0u8;size_of::<usize>()];
        let b:Option<NonNull<()>> = unsafe {std::mem::transmute(none_bytes)};
        assert!(b.is_none());
        let random_usize:usize = rng.gen_range(1..usize::MAX);
        let b:Option<NonNull<()>> = unsafe{std::mem::transmute(random_usize)};
        assert_eq!(b.map(|p| p.as_ptr()),Some(random_usize as *mut ()))
    }
}