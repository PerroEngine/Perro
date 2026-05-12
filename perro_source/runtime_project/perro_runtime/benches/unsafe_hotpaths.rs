use criterion::{Criterion, black_box, criterion_group, criterion_main};
use perro_input_api::{InputSnapshot, InputWindow, MouseMode};
use std::any::{Any, TypeId};

#[derive(Clone)]
struct BenchState {
    a: u64,
    b: u64,
    c: u64,
    d: u64,
}

impl BenchState {
    #[inline(always)]
    fn sum(&self) -> u64 {
        self.a
            .wrapping_add(self.b)
            .wrapping_add(self.c)
            .wrapping_add(self.d)
    }

    #[inline(always)]
    fn bump(&mut self) -> u64 {
        self.a = self.a.wrapping_add(1);
        self.sum()
    }
}

#[derive(Clone)]
struct OtherState {
    value: u64,
}

#[inline(always)]
fn safe_state_ref<T: 'static>(state: &dyn Any, expected: TypeId) -> Option<&T> {
    if expected != TypeId::of::<T>() {
        return None;
    }
    state.downcast_ref::<T>()
}

#[inline(always)]
fn safe_state_ref_only<T: 'static>(state: &dyn Any) -> Option<&T> {
    state.downcast_ref::<T>()
}

#[inline(always)]
fn unsafe_state_ref<T: 'static>(state: &dyn Any, expected: TypeId) -> Option<&T> {
    if expected != TypeId::of::<T>() {
        return None;
    }
    // SAFETY: Caller-supplied TypeId check matches T before pointer cast.
    Some(unsafe { &*(state as *const dyn Any as *const T) })
}

#[inline(always)]
fn safe_state_mut<T: 'static>(state: &mut dyn Any, expected: TypeId) -> Option<&mut T> {
    if expected != TypeId::of::<T>() {
        return None;
    }
    state.downcast_mut::<T>()
}

#[inline(always)]
fn safe_state_mut_only<T: 'static>(state: &mut dyn Any) -> Option<&mut T> {
    state.downcast_mut::<T>()
}

#[inline(always)]
fn unsafe_state_mut<T: 'static>(state: &mut dyn Any, expected: TypeId) -> Option<&mut T> {
    if expected != TypeId::of::<T>() {
        return None;
    }
    // SAFETY: Caller-supplied TypeId check matches T before pointer cast.
    Some(unsafe { &mut *(state as *mut dyn Any as *mut T) })
}

#[inline(always)]
fn mode_score(mode: MouseMode) -> u64 {
    match mode {
        MouseMode::Visible => 1,
        MouseMode::Hidden => 2,
        MouseMode::Captured => 3,
        MouseMode::Confined => 4,
        MouseMode::ConfinedHidden => 5,
    }
}

#[inline(always)]
fn direct_input_window(input: &InputSnapshot) -> u64 {
    let ipt = InputWindow::new(input);
    mode_score(ipt.Mouse().mode())
}

#[inline(always)]
fn unsafe_ptr_input_window(input: &InputSnapshot) -> u64 {
    let input_ptr = std::ptr::addr_of!(*input);
    // SAFETY: Pointer comes from live shared reference and is only read here.
    let ipt = unsafe { InputWindow::new(&*input_ptr) };
    mode_score(ipt.Mouse().mode())
}

fn bench_state_cast_ref(c: &mut Criterion) {
    let expected = TypeId::of::<BenchState>();
    let state: Box<dyn Any> = Box::new(BenchState {
        a: 1,
        b: 2,
        c: 3,
        d: 4,
    });
    let wrong_state: Box<dyn Any> = Box::new(OtherState { value: 7 });

    assert_eq!(
        safe_state_ref::<BenchState>(state.as_ref(), expected).map(BenchState::sum),
        unsafe_state_ref::<BenchState>(state.as_ref(), expected).map(BenchState::sum)
    );
    assert!(
        safe_state_ref::<BenchState>(wrong_state.as_ref(), TypeId::of::<OtherState>()).is_none()
    );
    assert!(
        unsafe_state_ref::<BenchState>(wrong_state.as_ref(), TypeId::of::<OtherState>()).is_none()
    );
    let wrong = safe_state_ref::<OtherState>(wrong_state.as_ref(), TypeId::of::<OtherState>())
        .map(|state| state.value);
    assert_eq!(wrong, Some(7));

    let mut group = c.benchmark_group("unsafe_hotpaths_state_ref");
    group.bench_function("safe_downcast_ref_hit", |b| {
        b.iter(|| {
            let state =
                safe_state_ref::<BenchState>(black_box(state.as_ref()), black_box(expected))
                    .expect("state type");
            black_box(state.sum())
        })
    });
    group.bench_function("safe_downcast_ref_only_hit", |b| {
        b.iter(|| {
            let state =
                safe_state_ref_only::<BenchState>(black_box(state.as_ref())).expect("state type");
            black_box(state.sum())
        })
    });
    group.bench_function("unsafe_typeid_cast_ref_hit", |b| {
        b.iter(|| {
            let state =
                unsafe_state_ref::<BenchState>(black_box(state.as_ref()), black_box(expected))
                    .expect("state type");
            black_box(state.sum())
        })
    });
    group.bench_function("safe_downcast_ref_miss", |b| {
        b.iter(|| {
            black_box(safe_state_ref::<BenchState>(
                black_box(wrong_state.as_ref()),
                black_box(TypeId::of::<OtherState>()),
            ))
        })
    });
    group.bench_function("safe_downcast_ref_only_miss", |b| {
        b.iter(|| {
            black_box(safe_state_ref_only::<BenchState>(black_box(
                wrong_state.as_ref(),
            )))
        })
    });
    group.bench_function("unsafe_typeid_cast_ref_miss", |b| {
        b.iter(|| {
            black_box(unsafe_state_ref::<BenchState>(
                black_box(wrong_state.as_ref()),
                black_box(TypeId::of::<OtherState>()),
            ))
        })
    });
    group.finish();
}

fn bench_state_cast_mut(c: &mut Criterion) {
    let expected = TypeId::of::<BenchState>();
    let mut group = c.benchmark_group("unsafe_hotpaths_state_mut");

    group.bench_function("safe_downcast_mut_hit", |b| {
        let mut state: Box<dyn Any> = Box::new(BenchState {
            a: 1,
            b: 2,
            c: 3,
            d: 4,
        });
        b.iter(|| {
            let state =
                safe_state_mut::<BenchState>(black_box(state.as_mut()), black_box(expected))
                    .expect("state type");
            black_box(state.bump())
        })
    });
    group.bench_function("safe_downcast_mut_only_hit", |b| {
        let mut state: Box<dyn Any> = Box::new(BenchState {
            a: 1,
            b: 2,
            c: 3,
            d: 4,
        });
        b.iter(|| {
            let state =
                safe_state_mut_only::<BenchState>(black_box(state.as_mut())).expect("state type");
            black_box(state.bump())
        })
    });
    group.bench_function("unsafe_typeid_cast_mut_hit", |b| {
        let mut state: Box<dyn Any> = Box::new(BenchState {
            a: 1,
            b: 2,
            c: 3,
            d: 4,
        });
        b.iter(|| {
            let state =
                unsafe_state_mut::<BenchState>(black_box(state.as_mut()), black_box(expected))
                    .expect("state type");
            black_box(state.bump())
        })
    });
    group.bench_function("safe_downcast_mut_miss", |b| {
        let mut state: Box<dyn Any> = Box::new(OtherState { value: 7 });
        b.iter(|| {
            let miss = safe_state_mut::<BenchState>(
                black_box(state.as_mut()),
                black_box(TypeId::of::<OtherState>()),
            )
            .is_none();
            black_box(miss)
        })
    });
    group.bench_function("safe_downcast_mut_only_miss", |b| {
        let mut state: Box<dyn Any> = Box::new(OtherState { value: 7 });
        b.iter(|| {
            let miss = safe_state_mut_only::<BenchState>(black_box(state.as_mut())).is_none();
            black_box(miss)
        })
    });
    group.bench_function("unsafe_typeid_cast_mut_miss", |b| {
        let mut state: Box<dyn Any> = Box::new(OtherState { value: 7 });
        b.iter(|| {
            let miss = unsafe_state_mut::<BenchState>(
                black_box(state.as_mut()),
                black_box(TypeId::of::<OtherState>()),
            )
            .is_none();
            black_box(miss)
        })
    });
    group.finish();
}

fn bench_input_window(c: &mut Criterion) {
    let input = InputSnapshot::new();
    assert_eq!(direct_input_window(&input), unsafe_ptr_input_window(&input));

    let mut group = c.benchmark_group("unsafe_hotpaths_input_window");
    group.bench_function("direct_shared_ref", |b| {
        b.iter(|| black_box(direct_input_window(black_box(&input))))
    });
    group.bench_function("unsafe_raw_ptr_shared_ref", |b| {
        b.iter(|| black_box(unsafe_ptr_input_window(black_box(&input))))
    });
    group.finish();
}

fn bench_unsafe_hotpaths(c: &mut Criterion) {
    bench_state_cast_ref(c);
    bench_state_cast_mut(c);
    bench_input_window(c);
}

criterion_group!(benches, bench_unsafe_hotpaths);
criterion_main!(benches);
