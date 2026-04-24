<!-- Test snippets: type @if, @foreach, {{ }} etc to trigger autocomplete -->
<!-- Test bracket closing: type {{ and verify }} auto-closes, {!! and verify !!} auto-closes -->

@extends('layouts.app')

@section('content')
    <div class="container">
        {{ $title }}

        @if($user)
            <p>Welcome, {{ $user->name }}!</p>
        @else
            <p>Please log in.</p>
        @endif

        @foreach($items as $item)
            <div class="item">
                {!! $item->description !!}
            </div>
        @endforeach

        @forelse($posts as $post)
            <article>
                <h2>{{ $post->title }}</h2>
                <p>{{ $post->excerpt }}</p>
            </article>
        @empty
            <p>No posts found.</p>
        @endforelse

        @switch($status)
            @case('active')
                <span class="badge-success">Active</span>
                @break
            @case('inactive')
                <span class="badge-danger">Inactive</span>
                @break
            @default
                <span class="badge-secondary">Unknown</span>
        @endswitch

        @auth
            <p>Authenticated user content</p>
        @endauth

        @guest
            <p>Guest user content</p>
        @endguest

        @can('edit-post')
            <a href="/posts/{{ $post->id }}/edit">Edit</a>
        @endcan

        @error('email')
            <span class="error">{{ $message }}</span>
        @enderror

        <form method="POST" action="/submit">
            @csrf
            @method('PUT')
            <input type="text" name="title" />
        </form>

        @component('components.alert', ['type' => 'info'])
            @slot('title')
                Important Notice
            @endslot

            This is an important message.
        @endcomponent
    </div>
@endsection

<?php
// PHP code block
class BlogController {
    public function index() {
        $posts = [];
        return view('blog.index', compact('posts'));
    }

    private function validateInput($data) {
        try {
            return true;
        } catch (Exception $e) {
            return false;
        }
    }
}
?>
