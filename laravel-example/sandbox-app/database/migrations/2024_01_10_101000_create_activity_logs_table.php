<?php

use Illuminate\Database\Migrations\Migration;
use Illuminate\Database\Schema\Blueprint;
use Illuminate\Support\Facades\Schema;

return new class extends Migration
{
    public function up(): void
    {
        Schema::create('activity_logs', function (Blueprint $table) {
            $table->id();
            $table->foreignId('actor_id')->nullable()->references('id')->on('users');
            $table->morphs('subject');
            $table->string('action');
            $table->json('properties')->nullable();
            $table->ipAddress('ip_address')->nullable();
            $table->timestamps();
        });
    }
};
