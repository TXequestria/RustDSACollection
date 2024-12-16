use crate::mybox::MyBox;
use std::ptr::NonNull;

type Ptr<T> = Option<NonNull<T>>;

//好吧，双向链表，Rust特有的超级期末考试
//双向链表几乎囊括了Rust里每一个基础知识，生命周期，所有权，线程安全，Marker, unsafe，迭代器.....
//如果你觉得你rust学的差不多了，那么就来看这个链表吧！

//双向链表最麻烦的一点就是，他是循环引用结构
//Node1 <--> Node2,两个指针会指向彼此
//Box、MyBox等持有所有权的变量是不能允许这种双向持有的，我们别无他法，只能使用裸指针
struct Node<T> {
    //这里用Option是为了方便调用.take() .as_ref() .as_mut()方法，比较方便
    //如果你用data:T的话，就要微操mem::read 或者mem::swap了，我们已经有足够的unsafe了...
    data:Option<T>,
    prev:Ptr<Self>,
    next:Ptr<Self>
}

//由于双向链表本身就是存储在堆内存上的，所以new函数直接一步到位写到堆上，得到一个裸指针
//这里用了我们之前写的MyBox，MyBox本质上调用的是malloc和free
impl<T> Node<T> {
    fn new(data:T) -> NonNull<Self> {
        let new_node = Self {
            data:Some(data),prev:None,next:None
        };
        MyBox::new(new_node).into_raw()
    }
    //drop_take是双向链表pop的时候用的，它会释放掉Node的内存，同时返回data的值
    //注意由于我们需要微操裸指针，所以这个函数是unsafe的！
    unsafe fn drop_take(ptr:NonNull<Self>) -> Option<T> {
        let mut boxed_node = MyBox::from_raw(ptr);
        let data = boxed_node.data.take();
        data
    }
}
//这玩意本质上就是Option<NonNull<Node<T>>>,但你写这么长谁看得懂啊？
type Link<T> = Ptr<Node<T>>;

//在讲解LinkedList以前，需要花点篇幅解释一下为什么LinkedList即使用了一堆裸指针，但它还是安全的
//与MyBox类似，LinkedList需要保证对堆内存的独占
//也就是说，LinkedList只要不把自己的head、tail指针暴露给别人
//就能保证只有自己能访问堆上的变量，LinkedList相当于担任了堆内存守门员的责任
//然而LinkedList比MyBox复杂很多，实际上Iter,IterMut,Cursor等辅助LinkedList的数据结构，都需要持有裸指针
//我们在后面会讨论如何保证总体的安全性
pub struct LinkedList<T> {
    head:Link<T>,
    tail:Link<T>,
    length:usize
}

impl<T> LinkedList<T> {
    pub fn new() -> Self {
        Self {
            head:None,tail:None,length:0
        }
    }

    //ExactSizeIterator也有一个len()方法，干脆是写到那里去了，避免重复
    //代码是一样的，只不过在很很很下面才实现
    // pub fn len(&self) -> usize {
    //     self.length
    // }

    //push、pop函数都是线程安全的
    //因为&mut self是独占的，即使在多线程环境下，rust编译器仍然能保证只有一个线程持有&mut self
    //而有人持有&mut的情况下，不仅别人不能持有&mut,甚至连&T也不能持有
    //rust的生命周期规则就是干这个的，因此持有&mut T的时候，我们可以放心大胆微操裸指针
    //不必担心是否有其他人也在微操裸指针，而导致race condition

    pub fn push_head(&mut self,data:T) {
        let mut new_node = Node::new(data);
        if self.len() == 0 {
            debug_assert!(self.head.is_none());
            debug_assert!(self.tail.is_none());

            //好吧，这需要一点panic safety的概念
            //简单来说，rust的panic不意味着程序会立刻终结，而是类似其他语言抛出exception一样
            //在panic发生的时候，所有作用域内的变量的drop函数都会被调用
            //rust的panic可以被catch，在多线程环境下，其他线程panic，也不会导致主线程挂掉
            //总之panic发生以后，程序还是可以活下来的
            //而panic safety，就是保证在修改变量内部元素的时候，不能panic
            //也就是说，panic只能在变量成功修改前发生，或者成功修改后发生，无论是否发生panic，我们的链表都应该有效
            //所以，我们应该把所有可能导致panic的操作（包括assert、unwarp、expect什么的），都要在开始修改链表以前干完

            //开始修改链表，接下来的部分不许出现panic
            self.head = Some(new_node);
            self.tail = Some(new_node);
            self.length = 1;
            return;
            //空链表的插入操作是把头尾指针都指向同一节点，然后设置长度=1
        }

        let mut old_head = self.head.expect("链表长度大于0, 链表头不应该为空");

        #[cfg(debug_assertions)]
        if self.len() == 1 {
            //长度为1时，头尾指针指向同一节点
            debug_assert_eq!(self.head,self.tail);
        }else{
            //长度大于1时，头尾指针指向不同的节点
            debug_assert_ne!(self.head,self.tail);
        }
        
        //开始修改链表，接下来的部分不许出现panic
        self.length += 1;
        unsafe {
            //NULL<-新头节点 <--> 老头节点, 老头节点的prev = 新头节点，新头节点的next=老头节点
            new_node.as_mut().next = Some(old_head);
            old_head.as_mut().prev = Some(new_node);
        }
        //修改链表头
        self.head = Some(new_node);
    }
    pub fn push_tail(&mut self,data:T) {
        //空链表push头、尾都一样
        if self.len() == 0 {
            self.push_head(data);
            return;
        }

        let mut new_node = Node::new(data);
        let mut old_tail = self.tail.expect("链表长度大于0, 链表尾不应该为空");
        //之所以直接expect了，是因为双向链表的有效性 (比如链表长度大于零时，尾巴不应该为空这样的保证)
        //是非常重要的保证，也叫internal consistency,双向链表的一切操作都建立在它是有效的前提上
        //如果双向链表的有效性被破坏了，我们几乎什么操作都无法进行，甚至drop函数都没有办法安全调用
        //此时我们应该直接panic，而不是试图让程序以奇怪的方式继续运行下去

        #[cfg(debug_assertions)]
        if self.len() == 1 {
            //长度为1时，头尾指针指向同一节点
            debug_assert_eq!(self.head,self.tail);
        }else{
            //长度大于1时，头尾指针指向不同的节点
            debug_assert_ne!(self.head,self.tail);
        }

        //开始修改链表，不许panic!

        self.length += 1;
        unsafe {
            //-->老尾节点<-->新尾节点->NULL
            old_tail.as_mut().next = Some(new_node);
            new_node.as_mut().prev = Some(old_tail);
        }
        self.tail = Some(new_node);
    }
    //头指针后移一格，我就不写注释了，感觉变量名还是很好懂的
    pub fn pop_head(&mut self) -> Option<T> {
        if self.len() == 0 {
            debug_assert!(self.head.is_none());
            debug_assert!(self.tail.is_none());
            return None;
        }

        let old_head = self.head.expect("长度不为0, 头指针应该存在");

        if self.len() == 1 {
            debug_assert_eq!(self.head,self.tail);

            //开始修改链表，不许panic!
            self.length = 0;
            self.head = None;
            self.tail = None;
            //drop_take在这里用上了，free掉old_head节点，同时取出节点里的值
            return unsafe{Node::drop_take(old_head)};
        }

        debug_assert_ne!(self.head,self.tail);

        let mut new_head = unsafe {old_head.as_ref()}.next.expect("长度大于1, 头指针的下一节点应该存在");

        //开始修改链表，接下来不许panic!
        self.length -= 1;
        //头指针后移一格
        self.head = Some(new_head);
        unsafe {
            //新头节点的prev应该置空
            new_head.as_mut().prev = None;
            //drop_take在这里用上了，free掉old_head节点，同时取出节点里的值
            Node::drop_take(old_head)
        }
    }
    pub fn pop_tail(&mut self) -> Option<T> {
        if self.len() <= 1 {
            return self.pop_head();
        }

        debug_assert_ne!(self.head,self.tail);
        let old_tail = self.tail.expect("长度大于0, 尾指针应该存在");
        let mut new_tail = unsafe{old_tail.as_ref()}.prev.expect("长度大于1, 尾指针的前置节点应该存在");

        //开始修改节点，不许panic!

        self.length -= 1;
        //尾指针前移一格
        self.tail = Some(new_tail);
        unsafe {
            //新尾节点的next应该置空
            //drop_take在这里用上了，free掉old_tail节点，同时取出节点里的值
            new_tail.as_mut().next = None;
            Node::drop_take(old_tail)
        }
    }
    pub fn peek_head(&self) -> Option<&T> {
        unsafe {
            self.head?.as_ref().data.as_ref()
        }
    }
    pub fn peek_tail(&self) -> Option<&T> {
        unsafe {
            self.tail?.as_ref().data.as_ref()
        }
    }
    pub fn peek_head_mut(&mut self) -> Option<&mut T> {
        unsafe {
            self.head?.as_mut().data.as_mut()
        }
    }
    pub fn peek_tail_mut(&mut self) -> Option<&mut T> {
        unsafe {
            self.tail?.as_mut().data.as_mut()
        }
    }
    pub fn clear(&mut self) {
        while let Some(_) = self.pop_head() {}
    }
}

//释放内存的drop函数，就，一直pop到没东西为止...
//因为Drop函数的实现本身就依赖pop函数，所以drop函数不能在链表本身损坏的时候工作
//而连drop函数都没法调用，很明显要么内存泄漏，要么abort，要么就UB了...
//因此在内部一致性被破坏的时候，最好的选择就是立即panic，你修也修不了的，越修越UB
impl<T> Drop for LinkedList<T> {
    fn drop(&mut self) {
        self.clear()
    }
}

use std::fmt::Debug;
impl<T: Debug> Debug for LinkedList<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_list().entries(self).finish()
    }
}

impl<T:Clone> Clone for LinkedList<T> {
    fn clone(&self) -> Self {
        let mut list = Self::new();
        for elem in self {
            list.push_tail(elem.clone());
        }
        list
    }
}

use std::hash::{Hash,Hasher};

impl<T:Hash> Hash for LinkedList<T> {
    fn hash<H:Hasher>(&self, state: &mut H) {
        self.len().hash(state);
        for i in self {
            i.hash(state)
        }
    }
}

impl<T> Default for LinkedList<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T:PartialEq> PartialEq for LinkedList<T> {
    fn eq(&self, other: &Self) -> bool {
        if self.len() != other.len() {
            return false;
        }
        for (x,y) in self.iter().zip(other.iter()) {
            if x != y {return false;}
        }
        return true;
    }
}

impl<T:Eq> Eq for LinkedList<T> {}

impl<T:PartialOrd> PartialOrd for LinkedList<T> {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        self.iter().partial_cmp(other)
    }
}

impl<T:Ord> Ord for LinkedList<T> {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.iter().cmp(other)
    }
}

impl<A> Extend<A> for LinkedList<A> {
    fn extend<T: IntoIterator<Item = A>>(&mut self, iter: T) {
        for elem in iter {
            self.push_tail(elem);
        }
    }
}

//迭代器相关的Trait，好累，不想详细解释了
//简单说一下吧，我们这个链表的顺序默认是头->尾的顺序
//所以正向迭代的时候，调用的是pop_head

use std::iter::{IntoIterator,Iterator,DoubleEndedIterator,FromIterator,ExactSizeIterator};
use std::marker::{PhantomData,Send,Sync};

impl<A> FromIterator<A> for LinkedList<A> 
{
    fn from_iter<T: IntoIterator<Item = A>>(iter: T) -> Self {
        let mut list = Self::new();
        for elem in iter {
            //这里用push_tail保证顺序和原迭代器相同
            list.push_tail(elem);
        }
        return list;
    }
}

impl<T> Iterator for LinkedList<T> {
    type Item = T;
    fn next(&mut self) -> Option<Self::Item> {
        //正向迭代，头先出
        self.pop_head()
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.len(), Some(self.len()))
    }
}

impl<T> ExactSizeIterator for LinkedList<T> {
    fn len(&self) -> usize {
        self.length
    }
}

//DoubleEndedIterator是标记这个迭代器也能反向迭代的Trait
//.reverse()方法就是基于这个Trait实现的
//作为双向链表要是不能反向迭代也太失败了
impl<T> DoubleEndedIterator for LinkedList<T> {
    fn next_back(&mut self) -> Option<Self::Item> {
        //反向迭代，尾巴先出
        self.pop_tail()
    }
}

//好了，下面是Iter、IterMut，分别对应不可变引用迭代器、可变引用迭代器
//由于他们要记录当前指向第几个节点，不可避免地要使用裸指针，如何保证整体的内存线程安全呢？
//答案是，PhantomData + 自律.....

//不可变引用迭代器，逻辑上这玩意是一个&LinkedList<T>
pub struct Iter<'a,T> {
    //我们使用一个PhantomData，来假装自己是一个&LinkedList<T>
    //这样编译器会意识到，持有一个Iter = 持有一个不可变引用
    //因此编译器会保证 1. Iter存在的时候，它指向的LinkedList必须也存在
    // 2. Iter存在的时候，不能存在&mut LinkedList, &mut T
    //如此保证不会发生竞争状态，也不会有野指针
    _m:PhantomData<&'a LinkedList<T>>,
    head:Link<T>,
    tail:Link<T>,
    length:usize
}

//迭代器：当迭代器返回None的时候，迭代停止
//为啥要含有head和tail呢？因为这个迭代器是可以双向迭代的，当头尾指针相遇的时候，迭代就终止啦
impl<'a,T> Iterator for Iter<'a,T> {
    type Item = &'a T;
    fn next(&mut self) -> Option<Self::Item> {
        let current = self.head?;
        if self.length == 0 {
            return None;
        }
        self.length -= 1;
        if self.head == self.tail {
            self.head = None;
            self.tail = None;
        }else{
            unsafe {self.head = current.as_ref().next;}
        }
        unsafe{current.as_ref().data.as_ref()}
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.len(), Some(self.len()))
    }
}
//这就是反向迭代的代码
//如果不实现这个trait，就没法实现.reverse()方法，你就没法很轻松的逆转迭代器了
impl<'a,T> DoubleEndedIterator for Iter<'a,T> {
    fn next_back(&mut self) -> Option<Self::Item> {
        //可以看到，这里的current取的是尾指针
        let current = self.tail?;
        if self.length == 0 {
            return None;
        }
        self.length -= 1;
        if self.head == self.tail {
            //头尾指针相遇，置空head、tail,下一回合迭代停止
            self.head = None;
            self.tail = None;
        }else{
            unsafe {self.tail = current.as_ref().prev;}
        }
        unsafe {current.as_ref().data.as_ref()}
    }
}

impl<'a,T> ExactSizeIterator for Iter<'a,T> {
    fn len(&self) -> usize {
        self.length
    }
}

impl<'a,T> IntoIterator for &'a LinkedList<T> {
    type IntoIter = Iter<'a,T>;
    type Item = &'a T;
    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

//可变引用迭代器，逻辑上这玩意是一个&mut LinkedList<T>
pub struct IterMut<'a,T> {
    //这个PhantomData是为了告诉编译器，逻辑上我占有一个&mut LinkedList,即使我的内容并不含有这个引用
    //如果你不加这个东西，编译器就没法跟踪生命周期，你就可以搞出很多个IterMut<'a,T>来，事实上破坏引用规则
    //加上这个东西以后，在IterMut存在的时候，编译器会保证：
    //1.它指向的LinkedList必须是还存在、还有效的
    //2.不会有其他的&T,&LinkedList,&mut T, &mut LinkedList,Iter,IterMut等指针/引用和你抢所有权
    //你可以安心访问堆内存上的变量，不会发生竞争状态，也不会有野指针
    _m:PhantomData<&'a mut LinkedList<T>>,
    head:Link<T>,
    tail:Link<T>,
    length:usize,
}

//迭代器：当迭代器返回None的时候，迭代停止
//为啥要含有head和tail呢？因为这个迭代器是可以双向迭代的，当头尾指针相遇的时候，迭代就终止啦
impl<'a,T> Iterator for IterMut<'a,T> {
    type Item = &'a mut T;
    fn next(&mut self) -> Option<Self::Item> {
        let mut current = self.head?;
        if self.length == 0 {
            return None;
        }
        self.length -= 1;
        if self.head == self.tail {
            //头尾指针相遇，置空指针，这样下一回合就会返回None，迭代就停止了
            self.head = None;
            self.tail = None;
        }else{
            //头指针后移一格
            unsafe {self.head = current.as_ref().next;}
        }
        unsafe{current.as_mut().data.as_mut()}
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.len(), Some(self.len()))
    }
}
//这就是反向迭代的代码
//如果不实现这个trait，就没法实现.reverse()方法，你就没法很轻松的逆转迭代器了
impl<'a,T> DoubleEndedIterator for IterMut<'a,T> {
    fn next_back(&mut self) -> Option<Self::Item> {
        //可以看到，这里的current取的是尾指针
        let mut current = self.tail?;
        if self.length == 0 {
            return None;
        }
        self.length -= 1;
        if self.head == self.tail {
            //头尾相遇，迭代终止，置空指针
            self.head = None;
            self.tail = None;
        }else{
            //不然的话，尾指针前移一格
            unsafe {self.tail = current.as_ref().prev;}
        }
        unsafe{current.as_mut().data.as_mut()}
    }
}

impl<'a,T> ExactSizeIterator for IterMut<'a,T> {
    fn len(&self) -> usize {
        self.length
    }
}

impl<T> LinkedList<T> {
    pub fn iter(&self) -> Iter<'_,T> {
        Iter { _m: PhantomData, head: self.head, tail: self.tail,length:self.len() }
    }
    pub fn iter_mut(&mut self) -> IterMut<'_,T> {
        IterMut { _m: PhantomData, head: self.head, tail: self.tail,length:self.len() }
    }
}

//对于Sync，Send，LinkedList 的表现应该和所持有的值T相同
unsafe impl<T:Send> Send for LinkedList<T> {}
unsafe impl<T:Sync> Sync for LinkedList<T> {}

//Iter本质上是一个LinkedList的不可变借用，应该表现得和&LinkedList<T>相同
//因为LinkedList<T>=T, 故此 Iter<'a,T> = &'a LinkedList<T> = &'a T,
//Send,Sync 的规则是, T:Sync <==> &T:Send, 不可变引用可以传递，&&T = &T, 故此 &T:Send, &&T:Send, &T:Sync
//简单且正确的写法是:
// unsafe impl<'a,T:Sync> Send for Iter<'a,T> {}
// unsafe impl<'a,T:Sync> Sync for Iter<'a,T> {}
//不过为了贯彻Iter<'a,T> 逻辑上就是一个 &'a LinkedList<T> 的教学原则, 我还是写成下面这样:
unsafe impl<'a,T> Send for Iter<'a,T> where &'a LinkedList<T>:Send {}
unsafe impl<'a,T> Sync for Iter<'a,T> where &'a LinkedList<T>:Sync {}

//IterMut本质上是一个LinkedList的可变引用，应该表现得和&mut LinkedList<T>相同
//&mut T是独占的，同一时刻只能有一个&mut T, 所以&mut T 的表现和T相同
//IterMut<'a,T> = &'a mut LinkedList<T> = &'a mut T = T
//简单且正确的写法是:
// unsafe impl<'a,T:Send> Send for IterMut<'a,T> {}
// unsafe impl<'a,T:Sync> Sync for IterMut<'a,T> {}
//不过为了贯彻IterMut<'a,T> 逻辑上就是一个 &'a mut LinkedList<T> 的教学原则, 我还是写成下面这样:
unsafe impl<'a,T> Send for IterMut<'a,T> where &'a mut LinkedList<T>:Send {}
unsafe impl<'a,T> Sync for IterMut<'a,T> where &'a mut LinkedList<T>:Sync {}

//Cursor, 游标, 游标其实到现在还没在std里稳定，官方有一堆神秘的rfc还没实现，我在这里面就简单起见，实现一些常用的功能
//反正你们也不可能在生产环境上我写的烂代码吧？要真上了我可不负责
//教学目的，就只实现了move_next,move_prev,pop_next和push_next
//游标指向节点，但是存在一个幽灵节点，即当链表为空，或者在头节点move_prev，在尾节点move_next就会进入幽灵节点
// 尾部 <--> 幽灵 <--> 头部 ..... 使得游标可以在链表上循环移动
// 幽灵节点，index = None
//在幽灵节点push/pop_next = push/pop_head
//在尾部push_next = 尾插入, 在尾部pop = pop幽灵节点 = 啥也不会发生，返回None
pub struct CursorMut<'a,T> {
    list:&'a mut LinkedList<T>,
    current:Link<T>,
    index:Option<usize>
}

impl<'a,T> CursorMut<'a,T> {
    pub fn index(&self) -> Option<usize> {
        self.index
    }
    pub fn move_next(&mut self) {
        if self.list.len() == 0 {
            debug_assert!(self.index.is_none());
            debug_assert!(self.current.is_none());
            return;
        }
        if self.index.is_none() {
            debug_assert!(self.current.is_none());
            self.index = Some(0);
            self.current = self.list.head;
            return;
        }
        let index = self.index.expect("不在幽灵节点, index不应为空");
        let current = self.current.expect("不在幽灵节点, current指针不应为空");
        let next_index = index + 1;
        let next_ptr = unsafe{current.as_ref().next};
        if next_index >= self.list.len() {
            debug_assert!(next_ptr.is_none());
            self.index = None;
            self.current = None;
            return;
        }
        self.index = Some(next_index);
        self.current = next_ptr;
    }
    pub fn move_prev(&mut self) {
        if self.list.len() == 0 {
            debug_assert!(self.index.is_none());
            debug_assert!(self.current.is_none());
            return;
        }
        if self.index.is_none() {
            debug_assert!(self.current.is_none());
            self.index = Some(self.list.len()-1);
            self.current = self.list.tail;
            return;
        }
        let index = self.index.unwrap();
        let current = self.current.unwrap();
        let prev_ptr = unsafe{current.as_ref().prev};
        if index == 0 {
            debug_assert!(prev_ptr.is_none());
            self.index = None;
            self.current = None;
            return;
        }
        self.index = Some(index-1);
        self.current = prev_ptr;
    }
    pub fn push_next(&mut self,data:T) {
        if self.index.is_none() {
            debug_assert!(self.current.is_none());
            self.list.push_head(data);
            return;
        }
        let index = self.index.expect("不在幽灵节点, index不应为空");
        let mut current = self.current.expect("不在幽灵节点, current指针不应为空");
        let next_index = index + 1;
        let next_ptr = unsafe{current.as_ref().next};
        if next_index >= self.list.len() {
            debug_assert!(next_ptr.is_none());
            self.list.push_tail(data);
            return;
        }
        let mut old_next = next_ptr.expect("不在尾节点, 下一节点指针应该存在");
        let mut new_next = Node::new(data);
        //开始修改链表，不能panic
        self.list.length += 1;
        unsafe {
            current.as_mut().next = Some(new_next);
            new_next.as_mut().prev = Some(current);
            new_next.as_mut().next = Some(old_next);
            old_next.as_mut().prev = Some(new_next);
        }

    }
    pub fn pop_next(&mut self) -> Option<T> {
        if self.list.len() == 0 {
            debug_assert!(self.index.is_none());
            debug_assert!(self.current.is_none());
            return None;
        }
        if self.index.is_none() {
            debug_assert!(self.current.is_none());
            return self.list.pop_head();
        }
        let index = self.index?;
        let mut current = self.current?;
        //在尾部
        if index + 1 >= self.list.len() {
            debug_assert!(unsafe{current.as_ref().next.is_none()});
            return None;
        }
        let old_next = unsafe{current.as_ref().next?};
        let Some(mut new_next) = (unsafe {old_next.as_ref().next}) 
        //next->next = NULL说明我们在尾指针的前一节点，此时直接pop——tail
        else {return self.list.pop_tail();};

        //上面用到了let-else语法，特方便说实话，比match，if let-else 甚至unwarp都方便
        // 语法 let ENUM(变量) = 变量 else {panic/continue/return}
        //等价于 let 变量 = match ENUM {变体1 {变量}, _ => {panic!/return/continue/break}} 
        // else 里面的东西是 发散 (!) 类型的，巨好用，强烈安利好吧

        //不在头部也不在尾部，current、old_next、new_next都不为空
        //开始修改链表，不许panic也不许提前返回
        self.list.length -= 1;
        unsafe {
            current.as_mut().next = Some(new_next);
            new_next.as_mut().prev = Some(current);
            Node::drop_take(old_next)
        }
    }
    pub fn peek(&self) -> Option<&T> {
        unsafe{self.current?.as_ref().data.as_ref()}
    }
    pub fn peek_mut(&mut self) -> Option<&mut T> {
        unsafe{self.current?.as_mut().data.as_mut()}
    }
}

impl<T> LinkedList<T> {
    pub fn cursor_mut(&mut self) -> CursorMut<'_,T> {
        CursorMut {list:self,current:None,index:None}
    }
}
//与IterMut相同
unsafe impl<'a,T> Send for CursorMut<'a,T> where &'a mut LinkedList<T>:Send {}
unsafe impl<'a,T> Sync for CursorMut<'a,T> where &'a mut LinkedList<T>:Sync {}

#[cfg(test)]
mod tests {
    use rand::Rng;

    use super::LinkedList;

    #[test]
    fn test_basic() {
        let size = 100;
        let mut list = LinkedList::new();
        for i in 0..size {
            assert_eq!(list.len(),i);
            list.push_head(i);
            assert_eq!(*list.peek_head().unwrap(),i)
        }
        for i in 0..size {
            assert_eq!(list.len(),size-i);
            assert_eq!(list.pop_tail().unwrap(),i)
        }
        for i in 0..size {
            assert_eq!(list.len(),i);
            list.push_tail(i);
            assert_eq!(*list.peek_tail().unwrap(),i)
        }
        for i in 0..size {
            assert_eq!(list.len(),size-i);
            assert_eq!(list.pop_head().unwrap(),i)
        }
        assert!(list.len() == 0);
        assert!(list.peek_head().is_none());
        assert!(list.peek_tail().is_none());
    }
    #[test]
    fn test_iter() {
        let mut rng = rand::thread_rng();
        let len = rng.gen_range(1024..4096);
        let mut array:Vec<i32> = (0..len).map(|_| {rng.gen()}).collect();
        let mut list:LinkedList<_> = array.iter().map(|i| {*i}).collect();
        assert_eq!(array.len(),list.len());
        for (x,y) in list.iter().zip(array.iter()) {
            assert_eq!(x,y)
        }
        for (x,y) in list.iter().rev().zip(array.iter().rev()) {
            assert_eq!(x,y)
        }
        for (x,y) in list.iter_mut().zip(array.iter_mut()) {
            assert_eq!(x,y)
        }
        for (x,y) in list.iter_mut().rev().zip(array.iter_mut().rev()) {
            assert_eq!(x,y)
        }
    }
    #[test]
    fn test_cursor() {
        let mut list:LinkedList<usize> = (0..100).collect();
        let mut cursor = list.cursor_mut();
        for i in 0..100 {
            cursor.move_next();
            assert_eq!(Some(&i),cursor.peek());
            assert_eq!(Some(i),cursor.index);
        }
        cursor.move_next();
        assert_eq!(None,cursor.peek());
        assert_eq!(None,cursor.index);
        for i in 0..100 {
            cursor.move_next();
            assert_eq!(Some(&i),cursor.peek());
            assert_eq!(Some(i),cursor.index);
        }
        cursor.move_next();
        assert_eq!(None,cursor.peek());
        for i in 99..=0 {
            cursor.move_prev();
            assert_eq!(Some(&i),cursor.peek());
            assert_eq!(Some(i),cursor.index);
        }
        cursor = list.cursor_mut();
        for _i in 0..100 {
            println!("{:?}",cursor.pop_next());
        }
        for i in 0..100 {
            cursor.push_next(i);
        }
        for _ in 0..50 {
            cursor.move_next();
        }
        for _ in 0..50 {
            cursor.push_next(0);
        }
        drop(cursor);
        println!("{list:?},{}",list.len())
    }
}