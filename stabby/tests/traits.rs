//
// Copyright (c) 2023 ZettaScale Technology
//
// This program and the accompanying materials are made available under the
// terms of the Eclipse Public License 2.0 which is available at
// http://www.eclipse.org/legal/epl-2.0, or the Apache License, Version 2.0
// which is available at https://www.apache.org/licenses/LICENSE-2.0.
//
// SPDX-License-Identifier: EPL-2.0 OR Apache-2.0
//
// Contributors:
//   ZettaScale Zenoh Team, <zenoh@zettascale.tech>
//

// MYTRAIT

#![cfg_attr(feature = "unsafe_wakers", allow(deprecated))]

use std::time::Duration;

use stabby::future::DynFuture;

#[stabby::stabby]
pub trait MyTrait {
    type Output;
    extern "C" fn do_stuff<'a>(&'a self, with: &'a Self::Output) -> &'a u8;
    extern "C" fn gen_stuff(&mut self) -> Self::Output;
}

// IMPL

impl MyTrait for u8 {
    type Output = u8;
    extern "C" fn do_stuff<'a>(&'a self, _: &'a Self::Output) -> &'a u8 {
        self
    }
    extern "C" fn gen_stuff(&mut self) -> Self::Output {
        *self
    }
}
impl MyTrait for u16 {
    type Output = u8;
    extern "C" fn do_stuff<'a>(&'a self, _: &'a Self::Output) -> &'a u8 {
        &0
    }
    extern "C" fn gen_stuff(&mut self) -> Self::Output {
        *self as u8
    }
}

// MYTRAIT2
#[stabby::stabby]
pub trait MyTrait2 {
    extern "C" fn do_stuff2(&self) -> u8;
}

// IMPL

impl MyTrait2 for u8 {
    extern "C" fn do_stuff2(&self) -> u8 {
        *self
    }
}
impl MyTrait2 for u16 {
    extern "C" fn do_stuff2(&self) -> u8 {
        (*self) as u8
    }
}

#[stabby::stabby]
pub trait MyTrait3<Hi: core::ops::Deref> {
    type A;
    type B;
    extern "C" fn do_stuff3<'a>(&'a self, a: &'a Self::A, b: Self::B) -> Self::B;
    extern "C" fn gen_stuff3(&mut self, with: Hi) -> Self::A;
}

impl MyTrait3<Box<()>> for u8 {
    type A = u8;
    type B = u8;
    extern "C" fn do_stuff3<'a>(&'a self, _a: &'a Self::A, _b: Self::B) -> Self::B {
        *self
    }
    extern "C" fn gen_stuff3(&mut self, _with: Box<()>) -> Self::A {
        *self
    }
}
impl MyTrait3<Box<()>> for u16 {
    type A = u8;
    type B = u8;
    extern "C" fn do_stuff3<'a>(&'a self, _a: &'a Self::A, _b: Self::B) -> Self::B {
        (*self) as u8
    }
    extern "C" fn gen_stuff3(&mut self, _with: Box<()>) -> Self::A {
        (*self) as u8
    }
}

#[stabby::stabby]
pub trait AsyncRead {
    extern "C" fn read<'a>(
        &'a mut self,
        buffer: stabby::slice::SliceMut<'a, u8>,
    ) -> stabby::future::DynFuture<'a, usize>;
}
impl<'b> AsyncRead for stabby::slice::Slice<'b, u8> {
    extern "C" fn read<'a>(
        &'a mut self,
        mut buffer: stabby::slice::SliceMut<'a, u8>,
    ) -> stabby::future::DynFuture<'a, usize> {
        Box::new(async move {
            let len = self.len().min(buffer.len());
            let (l, r) = self.split_at(len);
            let r = unsafe { core::mem::transmute::<_, &[u8]>(r) };
            buffer[..len].copy_from_slice(l);
            *self = r.into();
            len
        })
        .into()
    }
}

#[test]
fn dyn_traits() {
    let boxed = Box::new(6u8);
    let mut dyned = <stabby::dynptr!(
        Box<dyn Send + MyTrait2 + MyTrait3<Box<()>, A = u8, B = u8> + Sync + MyTrait<Output = u8>>
    )>::from(boxed);
    assert_eq!(dyned.downcast_ref::<u8>(), Some(&6));
    assert_eq!(dyned.do_stuff(&0), &6);
    assert_eq!(dyned.gen_stuff(), 6);
    assert_eq!(dyned.gen_stuff3(Box::new(())), 6);
    assert!(dyned.downcast_ref::<u16>().is_none());
    fn trait_assertions<T: Send + Sync + stabby::abi::IStable>(_t: T) {}
    trait_assertions(dyned);
}

#[test]
fn async_trait() {
    let (tx, rx) = smol::channel::bounded(5);
    let read_task = async move {
        let mut expected = 0;
        while let Ok(i) = rx.recv().await {
            assert_eq!(i, expected);
            expected += 1;
        }
        assert_eq!(expected, 5)
    };
    let write_task = async move {
        for i in 0..5 {
            tx.send(i).await.unwrap();
            smol::Timer::after(Duration::from_millis(30)).await;
        }
    };
    fn check(read: DynFuture<'static, ()>, write: DynFuture<'static, ()>) {
        let rtask = smol::spawn(read);
        let wtask = smol::spawn(write);
        smol::block_on(smol::future::zip(rtask, wtask));
    }
    check(Box::new(read_task).into(), Box::new(write_task).into())
}
